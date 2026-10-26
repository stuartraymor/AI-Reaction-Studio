use crate::avatar::Pose;
use crate::style::Mood;
use crate::{Analysis, Progress, Result, Script, TICKS, digest, exec, id, read_capped, save_json, validate_script};
use filmcraft_project::ClipId;
use filmcraft_time::{FrameRate, Tick};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn shape(s: &mut filmcraft_engine::Session, clip: Option<u64>, pos: [i32; 2], size: [i32; 2], color: &str, start: Tick, duration: Tick) -> Result<(u64, u64)> {
    let mut p = json!({"shape":"ellipse","position":pos,"size":size,"time":start.0,"seconds":duration.seconds(),"track":1});
    if let Some(c) = clip {
        p["clip"] = json!(c);
    }
    let r = exec(s, "graphics.newShape", p)?;
    let c = id(&r, "clip")?;
    let l = id(&r, "layer")?;
    exec(s, "graphics.set", json!({"clip":c,"layer":l,"props":{"fill_color":color}}))?;
    Ok((c, l))
}
/// The figure's layers, in creation order on one graphic clip, and their neutral positions/sizes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Part {
    Body,
    Eye(usize),
    Brow(usize),
    Mouth,
}
const BODY: [f64; 2] = [550.0, 275.0];
const EYES: [[f64; 2]; 2] = [[530.0, 258.0], [572.0, 258.0]];
const BROWS: [[f64; 2]; 2] = [[530.0, 240.0], [572.0, 240.0]];
const MOUTH: [f64; 2] = [550.0, 302.0];
const INK: &str = "#171126";
/// Every (layer, param, value) to keyframe for one pose. `face` false = mouth only (the mouth is
/// sampled twice as often as the rest of the face to follow speech).
pub(crate) fn face_keys(pose: &Pose, mouth: f64, face: bool) -> Vec<(Part, &'static str, Value)> {
    let bob = f64::from(pose.bob);
    let mut keys = vec![(Part::Mouth, "position", json!([MOUTH[0], MOUTH[1] + bob])), (Part::Mouth, "size", json!([38.0, mouth]))];
    if face {
        keys.push((Part::Body, "position", json!([BODY[0], BODY[1] + bob])));
        for (i, (eye, brow)) in EYES.iter().zip(BROWS.iter()).enumerate() {
            let raise = f64::from(pose.brow_raise.get(i).copied().unwrap_or(0.0));
            keys.push((Part::Eye(i), "position", json!([eye[0], eye[1] + bob])));
            keys.push((Part::Eye(i), "size", json!([15.0, (20.0 * f64::from(pose.eye_open)).max(2.0)])));
            keys.push((Part::Brow(i), "position", json!([brow[0], brow[1] + bob - raise])));
            keys.push((Part::Brow(i), "rotation", json!(f64::from(pose.brow_tilt.get(i).copied().unwrap_or(0.0)))));
        }
    }
    keys
}
fn layer_effect(s: &filmcraft_engine::Session, clip: u64, layer: u64) -> Result<usize> {
    s.active_sequence()
        .and_then(|q| q.find_item(ClipId(clip)))
        .and_then(|(_, c)| filmcraft_project::graphic::layer_indices(&c.effects).get(usize::try_from(layer).ok()?).copied())
        .ok_or_else(|| "figure layer not found".into())
}
#[allow(clippy::too_many_arguments)]
fn avatar(s: &mut filmcraft_engine::Session, start: Tick, hold: Tick, voice: &Value, color: &str, mood: Mood, gain: f32, p: &Progress) -> Result<()> {
    let (c, body) = shape(s, None, [550, 275], [120, 130], color, start, hold)?;
    let mut layers = vec![(Part::Body, body)];
    for (i, [x, y]) in EYES.iter().enumerate() {
        layers.push((Part::Eye(i), shape(s, Some(c), [*x as i32, *y as i32], [15, 20], "#ffffff", start, hold)?.1));
    }
    for (i, [x, y]) in BROWS.iter().enumerate() {
        layers.push((Part::Brow(i), shape(s, Some(c), [*x as i32, *y as i32], [22, 5], INK, start, hold)?.1));
    }
    layers.push((Part::Mouth, shape(s, Some(c), [550, 302], [38, 5], INK, start, hold)?.1));
    let mut effects = Vec::new();
    for (part, layer) in layers {
        effects.push((part, layer_effect(s, c, layer)?));
    }
    let wav = read_capped(Path::new(voice.get("path").and_then(Value::as_str).ok_or("voice path missing")?), 64_000_000)?;
    if wav.get(36..40) != Some(b"data") {
        return Err("unexpected narration WAV layout".into());
    }
    let rate = id(voice, "sampleRate")?;
    if rate == 0 || rate > 192000 {
        return Err("invalid narration sample rate".into());
    }
    let samples: Vec<f32> = wav.get(44..).ok_or("narration truncated")?.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    if samples.iter().any(|x| !x.is_finite()) {
        return Err("invalid voice samples".into());
    }
    let frame = FrameRate { num: 24, den: 1 }.frame_duration();
    for f in (0..hold.0 / frame.0).step_by(2) {
        if f % 120 == 0 {
            p.step("Animating the reacting figure")?;
        }
        let n = usize::try_from(Tick(f * frame.0).to_units_floor(rate as i64))?;
        let window = samples.get(n..n.saturating_add(rate as usize / 12).min(samples.len())).unwrap_or(&[]);
        let rms = if window.is_empty() { 0.0 } else { (window.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / window.len() as f64).sqrt() };
        let at = start + Tick(f * frame.0);
        let pose = crate::avatar::pose(mood, gain, Tick(f * frame.0).seconds());
        for (part, param, value) in face_keys(&pose, crate::avatar::mouth_height(rms, gain), f % 4 == 0) {
            let effect = effects.iter().find(|(p, _)| *p == part).map(|(_, e)| *e).ok_or("figure layer missing")?;
            exec(s, "effects.setParam", json!({"clip":c,"effect":effect,"param":param,"value":value,"time":at.0,"keyframe":true}))?;
        }
    }
    Ok(())
}
pub fn render(a: &Analysis, script: &Script, out: &Path, voice: &str, character: &str, approved: bool, p: &Progress) -> Result<()> {
    render_with_subtitles(a, script, out, voice, character, approved, false, p)
}
pub fn render_with_subtitles(
    a: &Analysis,
    script: &Script,
    out: &Path,
    voice: &str,
    character: &str,
    approved: bool,
    subtitles: bool,
    p: &Progress,
) -> Result<()> {
    if !approved {
        return Err("review and approve the generated script before rendering".into());
    }
    validate_script(a, script)?;
    if digest(&a.source)? != a.source_sha256 {
        return Err("source video changed since analysis; analyze it again".into());
    }
    if !["kokoro-heart", "kokoro-michael", "kokoro-puck"].contains(&voice) {
        return Err("unknown voice".into());
    }
    let color = match character {
        "Violet" => "#704de8",
        "Teal" => "#087f82",
        _ => return Err("unknown character".into()),
    };
    fs::create_dir_all(out)?;
    if out.join("reaction.fcproj").exists() {
        return Err("render folder already contains a saved project; choose a new folder".into());
    }
    let out = &fs::canonicalize(out)?;
    let mut approved_script = script.clone();
    approved_script.review_required = false;
    save_json(&out.join("approved-script.json"), &approved_script)?;
    p.step("Building the editable reaction timeline")?;
    let mut s = filmcraft_engine::Session::default();
    exec(&mut s, "file.newProject", json!({"name":"AI Reaction Studio"}))?;
    let r = exec(&mut s, "file.import", json!({"paths":[a.source]}))?;
    if r.get("errors").and_then(Value::as_array).is_some_and(|x| !x.is_empty()) {
        return Err(format!("render import failed: {r}").into());
    }
    let item = r.get("items").and_then(Value::as_array).and_then(|x| x.first()).and_then(Value::as_u64).ok_or("source not imported")?;
    exec(&mut s, "file.newSequence", json!({"name":"AI Reaction Studio","width":640,"height":360,"fps":24,"video":3,"audio":2}))?;
    if subtitles {
        exec(&mut s, "captions.newTrack", json!({"name":"Reaction commentary (estimated timing)","language":"en"}))?;
        exec(&mut s, "captions.setStyle", json!({"track":"C1","size":20,"background":true,"anchor":"bottom","margin":0.40,"outline":1}))?;
    }
    let (mood, gain) = crate::style::motion(&script.style, &script.perspective, script.intensity);
    let frame = FrameRate { num: 24, den: 1 }.frame_duration();
    let rate = FrameRate { num: 24, den: 1 };
    let mut source_pos = Tick::ZERO;
    let mut timeline_pos = Tick::ZERO;
    let mut timing = Vec::new();
    for (i, r) in script.segments.iter().enumerate() {
        p.step(&format!("Synthesizing reaction {} of {} locally", i + 1, script.segments.len()))?;
        let at = rate.snap_nearest(Tick(r.at)).min(Tick(a.duration) - frame);
        if at <= source_pos {
            return Err("reaction timestamps are too close after frame snapping".into());
        }
        let play = at - source_pos;
        exec(&mut s, "timeline.place", json!({"item":item,"track":"V1","audioTrack":"A1","sourceIn":source_pos.0,"time":timeline_pos.0,"duration":play.0}))?;
        timeline_pos += play;
        let narration = exec(&mut s, "tts.create", json!({"text":r.commentary,"voice":voice,"time":timeline_pos.0,"track":"A2","dir":out.join("assets")}))?;
        let speech = narration.get("duration").and_then(Value::as_i64).ok_or("narration duration missing")?;
        if speech <= 0 || speech > 120 * TICKS {
            return Err("narration duration exceeds two-minute segment limit".into());
        }
        let hold = Tick(speech.checked_add(frame.0 - 1).ok_or("speech duration overflow")? / frame.0 * frame.0);
        let held = exec(&mut s, "timeline.place", json!({"item":item,"track":"V1","audio":false,"sourceIn":at.0,"time":timeline_pos.0,"duration":frame.0}))?;
        let hc = held.get("clips").and_then(Value::as_array).and_then(|x| x.first()).and_then(Value::as_u64).ok_or("hold clip missing")?;
        exec(&mut s, "clip.frameHoldOptions", json!({"clips":[hc],"holdOn":"in"}))?;
        exec(&mut s, "timeline.trim", json!({"clip":hc,"edge":"out","mode":"regular","delta":(hold-frame).0}))?;
        avatar(&mut s, timeline_pos, hold, &narration, color, mood, gain, p)?;
        if subtitles {
            crate::subtitles::add(&mut s, &r.commentary, timeline_pos, Tick(speech))?;
        }
        exec(
            &mut s,
            "graphics.newText",
            json!({"text":format!("{} reaction • opinion",script.perspective),"position":[20,32],"size":19,"time":timeline_pos.0,"seconds":hold.seconds(),"track":2,"newClip":true}),
        )?;
        timing.push(json!({"evidence_id":r.evidence_id,"sourceTimestampTicks":r.at,"snappedSourceTimestampTicks":at.0,"timelineStartTicks":timeline_pos.0,"speechDurationTicks":speech,"holdDurationTicks":hold.0,"sourceQuote":r.source_quote,"commentary":r.commentary}));
        timeline_pos += hold;
        source_pos = at;
    }
    let remaining = rate.snap_nearest(Tick(a.duration)) - source_pos;
    if remaining > Tick::ZERO {
        exec(
            &mut s,
            "timeline.place",
            json!({"item":item,"track":"V1","audioTrack":"A1","sourceIn":source_pos.0,"time":timeline_pos.0,"duration":remaining.0}),
        )?;
    }
    if subtitles {
        exec(&mut s, "captions.export", json!({"path":out.join("reaction.srt"),"track":"C1"}))?;
        save_json(&out.join("captions.json"), &exec(&mut s, "captions.list", json!({}))?)?;
    }
    exec(&mut s, "file.saveAs", json!({"path":out.join("reaction.fcproj")}))?;
    let before = exec(&mut s, "sequence.inspect", json!({}))?;
    save_json(&out.join("timeline.json"), &before)?;
    save_json(&out.join("reaction-timing.json"), &timing)?;
    save_json(&out.join("analysis.json"), a)?;
    p.step("Rendering H.264/AAC MP4 with FilmCraft; project already saved")?;
    let job = exec(
        &mut s,
        "file.exportMedia",
        json!({"path":out.join("reaction.mp4"),"format":"h264","hardwareEncoding":"off","width":640,"height":360,"fps":24,"burnCaptions":subtitles,"wait":false}),
    )?;
    let job_id = id(&job, "job")?;
    loop {
        let job = s.jobs.iter().find(|j| j.id == job_id).ok_or("export job disappeared")?;
        if p.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            job.progress.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if job.progress.finished.load(std::sync::atomic::Ordering::Relaxed) {
            let result = job.result.lock().unwrap_or_else(|e| e.into_inner());
            match result.as_ref() {
                Some(Ok(_)) => break,
                Some(Err(e)) => return Err(format!("export failed: {e}").into()),
                None => return Err("export finished without a result".into()),
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    p.step("Reopening the saved project to verify editability")?;
    let mut reopened = filmcraft_engine::Session::default();
    let opened = exec(&mut reopened, "file.open", json!({"path":out.join("reaction.fcproj")}))?;
    if opened.get("missingMedia").and_then(Value::as_u64).unwrap_or(0) > 0 {
        return Err("saved project has missing media".into());
    }
    let after = exec(&mut reopened, "sequence.inspect", json!({}))?;
    if subtitles {
        let original_captions = exec(&mut s, "captions.list", json!({}))?;
        let reopened_captions = exec(&mut reopened, "captions.list", json!({}))?;
        if original_captions != reopened_captions {
            return Err("caption tracks changed on project reopen".into());
        }
        save_json(&out.join("reopened-captions.json"), &reopened_captions)?;
    }
    if ["video", "audio", "duration", "durationFrames"].iter().any(|key| before.get(key) != after.get(key)) {
        return Err("saved project changed on reopen".into());
    }
    save_json(&out.join("reopened-timeline.json"), &after)?;
    save_json(
        &out.join("production.json"),
        &json!({"complete":true,"source_sha256":a.source_sha256,"script_sha256":digest(&out.join("approved-script.json"))?,"cloudUploads":0,"voice":voice,"character":character,"style":script.style,"mood":format!("{mood:?}"),"commentarySubtitles":subtitles,"captionTiming":"estimated from narration duration and word counts","editingStyle":"Pause and React","projectReopenVerified":true}),
    )?;
    p.step("Reaction video and editable project are ready")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avatar::pose;
    const MOODS: [Mood; 5] = [Mood::Calm, Mood::Excited, Mood::Skeptical, Mood::Amused, Mood::Fierce];
    #[test]
    fn face_keys_cover_every_layer_and_stay_on_screen() {
        for mood in MOODS {
            for i in 0..200 {
                let keys = face_keys(&pose(mood, 1.0, i as f64 * 0.05), 26.0, true);
                for part in [Part::Body, Part::Eye(0), Part::Eye(1), Part::Brow(0), Part::Brow(1), Part::Mouth] {
                    assert!(keys.iter().any(|(p, _, _)| *p == part), "{part:?}");
                }
                for (_, param, value) in &keys {
                    let nums: Vec<f64> =
                        value.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_else(|| value.as_f64().into_iter().collect());
                    assert!(!nums.is_empty() && nums.iter().all(|v| v.is_finite()), "{param} {value}");
                    if *param == "position" {
                        assert!((0.0..=640.0).contains(&nums[0]) && (0.0..=360.0).contains(&nums[1]), "{value}");
                    }
                }
            }
        }
    }
    #[test]
    fn mouth_only_keys_skip_the_rest_of_the_face() {
        let keys = face_keys(&crate::avatar::NEUTRAL, 4.0, false);
        assert!(keys.iter().all(|(p, _, _)| *p == Part::Mouth) && keys.len() == 2);
    }
    #[test]
    fn eyes_never_close_to_zero_height() {
        let closed = Pose { eye_open: 0.0, ..crate::avatar::NEUTRAL };
        let eye = face_keys(&closed, 4.0, true).into_iter().find(|(p, k, _)| *p == Part::Eye(0) && *k == "size").unwrap().2;
        assert!(eye[1].as_f64().unwrap() >= 2.0);
    }
}

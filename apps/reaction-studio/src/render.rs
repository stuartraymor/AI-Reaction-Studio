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
fn avatar(s: &mut filmcraft_engine::Session, start: Tick, hold: Tick, voice: &Value, color: &str, p: &Progress) -> Result<()> {
    let (c, _) = shape(s, None, [550, 275], [120, 130], color, start, hold)?;
    shape(s, Some(c), [530, 258], [15, 20], "#ffffff", start, hold)?;
    shape(s, Some(c), [572, 258], [15, 20], "#ffffff", start, hold)?;
    let (_, mouth) = shape(s, Some(c), [550, 302], [38, 5], "#171126", start, hold)?;
    let effect = s
        .active_sequence()
        .and_then(|q| q.find_item(ClipId(c)))
        .and_then(|(_, c)| filmcraft_project::graphic::layer_indices(&c.effects).get(mouth as usize).copied())
        .ok_or("mouth layer not found")?;
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
            p.step("Animating speech-driven mouth keyframes")?;
        }
        let n = usize::try_from(Tick(f * frame.0).to_units_floor(rate as i64))?;
        let window = samples.get(n..n.saturating_add(rate as usize / 12).min(samples.len())).unwrap_or(&[]);
        let rms = if window.is_empty() { 0.0 } else { (window.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / window.len() as f64).sqrt() };
        exec(
            s,
            "effects.setParam",
            json!({"clip":c,"effect":effect,"param":"size","value":[38.0,(4.0+rms*120.0).clamp(4.0,26.0)],"time":(start+Tick(f*frame.0)).0,"keyframe":true}),
        )?;
    }
    Ok(())
}
pub fn render(a: &Analysis, script: &Script, out: &Path, voice: &str, character: &str, approved: bool, p: &Progress) -> Result<()> {
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
        avatar(&mut s, timeline_pos, hold, &narration, color, p)?;
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
    exec(&mut s, "file.saveAs", json!({"path":out.join("reaction.fcproj")}))?;
    let before = exec(&mut s, "sequence.inspect", json!({}))?;
    save_json(&out.join("timeline.json"), &before)?;
    save_json(&out.join("reaction-timing.json"), &timing)?;
    save_json(&out.join("analysis.json"), a)?;
    p.step("Rendering H.264/AAC MP4 with FilmCraft; project already saved")?;
    let job = exec(
        &mut s,
        "file.exportMedia",
        json!({"path":out.join("reaction.mp4"),"format":"h264","hardwareEncoding":"off","width":640,"height":360,"fps":24,"wait":false}),
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
    if ["video", "audio", "duration", "durationFrames"].iter().any(|key| before.get(key) != after.get(key)) {
        return Err("saved project changed on reopen".into());
    }
    save_json(&out.join("reopened-timeline.json"), &after)?;
    save_json(
        &out.join("production.json"),
        &json!({"complete":true,"source_sha256":a.source_sha256,"script_sha256":digest(&out.join("approved-script.json"))?,"cloudUploads":0,"voice":voice,"character":character,"editingStyle":"Pause and React","projectReopenVerified":true}),
    )?;
    p.step("Reaction video and editable project are ready")?;
    Ok(())
}

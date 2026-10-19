//! Local, deterministic acceptance prototype. No AI analysis or cloud calls.
use filmcraft_engine::Session;
use filmcraft_time::{FrameRate, TICKS_PER_SECOND, Tick};
use serde_json::{Value, json};
use std::{error::Error, fs, io::Read, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn read_capped(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(format!("input exceeds {limit} byte limit: {}", path.display()).into());
    }
    Ok(bytes)
}

fn run(s: &mut Session, id: &str, p: Value) -> Result<Value> {
    eprintln!("{id}");
    Ok(s.execute(id, p).map_err(|e| format!("{id}: {e}"))?)
}
fn number(v: &Value, key: &str) -> Result<u64> {
    v.get(key).and_then(Value::as_u64).ok_or_else(|| format!("missing {key}: {v}").into())
}
fn shape(s: &mut Session, clip: Option<u64>, pos: [i32; 2], size: [i32; 2], color: &str, start: Tick, duration: Tick) -> Result<(u64, u64)> {
    let mut p = json!({"shape":"ellipse", "position":pos, "size":size, "time":start.0, "seconds":duration.seconds(), "track":1});
    if let Some(c) = clip {
        p["clip"] = json!(c);
    }
    let r = run(s, "graphics.newShape", p)?;
    let c = number(&r, "clip")?;
    let l = number(&r, "layer")?;
    run(s, "graphics.set", json!({"clip":c,"layer":l,"props":{"fill_color":color}}))?;
    Ok((c, l))
}

fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let source = a.get(1).ok_or("usage: reaction_prototype SOURCE.mp4 OUTPUT_DIR [SCRIPT.txt]")?;
    let out = Path::new(a.get(2).ok_or("missing output directory")?);
    fs::create_dir_all(out)?;
    let out = out.canonicalize()?;
    if out.join("reaction.fcproj").exists() {
        return Err("output already contains a project; choose a new directory".into());
    }
    let source = Path::new(source).canonicalize()?;
    let text = match a.get(3) {
        Some(p) => String::from_utf8(read_capped(Path::new(p), 1000)?)?,
        None => "Let's pause and consider what we just saw. This is a local reaction test.".into(),
    };
    if text.trim().is_empty() || text.len() > 1000 {
        return Err("script must contain 1–1000 bytes".into());
    }
    let voice_id = std::env::var("REACTION_VOICE").unwrap_or_else(|_| "kokoro-heart".into());
    let mut s = Session::default();
    run(&mut s, "file.newProject", json!({"name":"AI Reaction Studio — local prototype"}))?;
    let imp = run(&mut s, "file.import", json!({"paths":[source]}))?;
    if imp.get("errors").and_then(Value::as_array).is_some_and(|e| !e.is_empty()) {
        return Err(format!("import errors: {imp}").into());
    }
    let item = imp.get("items").and_then(Value::as_array).and_then(|x| x.first()).and_then(Value::as_u64).ok_or("source not imported")?;
    let duration = s.project.item(filmcraft_project::ItemId(item)).ok_or("imported item missing")?.duration();
    let rate = FrameRate { num: 24, den: 1 };
    let split = Tick(2 * TICKS_PER_SECOND);
    if duration < Tick(3 * TICKS_PER_SECOND) || duration > Tick(60 * TICKS_PER_SECOND) {
        return Err("prototype source must be 3–60 seconds".into());
    }
    run(&mut s, "file.newSequence", json!({"name":"Pause and React", "width":640,"height":360,"fps":24,"video":3,"audio":2}))?;
    let voice = run(&mut s, "tts.create", json!({"text":text,"voice":voice_id,"time":split.0,"track":"A2","dir":out.join("assets")}))?;
    let speech = voice.get("duration").and_then(Value::as_i64).map(Tick).ok_or("narration has no duration")?;
    let frame = rate.frame_duration();
    let hold = Tick(speech.0.checked_add(frame.0 - 1).ok_or("duration overflow")? / frame.0 * frame.0);
    run(&mut s, "timeline.place", json!({"item":item,"track":"V1","audioTrack":"A1","time":0,"sourceIn":0,"duration":split.0}))?;
    let held = run(&mut s, "timeline.place", json!({"item":item,"track":"V1","audio":false,"time":split.0,"sourceIn":split.0,"duration":frame.0}))?;
    let hc = held.get("clips").and_then(Value::as_array).and_then(|x| x.first()).and_then(Value::as_u64).ok_or("hold clip missing")?;
    run(&mut s, "clip.frameHoldOptions", json!({"clips":[hc],"holdOn":"in"}))?;
    run(&mut s, "timeline.trim", json!({"clip":hc,"edge":"out","mode":"regular","delta":(hold-frame).0}))?;
    run(&mut s, "timeline.place", json!({"item":item,"track":"V1","audioTrack":"A1","time":(split+hold).0,"sourceIn":split.0,"duration":(duration-split).0}))?;
    let (avatar, _) = shape(&mut s, None, [550, 275], [120, 130], "#704de8", split, hold)?;
    shape(&mut s, Some(avatar), [530, 258], [15, 20], "#ffffff", split, hold)?;
    shape(&mut s, Some(avatar), [572, 258], [15, 20], "#ffffff", split, hold)?;
    let (_, mouth) = shape(&mut s, Some(avatar), [550, 302], [38, 5], "#171126", split, hold)?;
    let effect_index = s
        .active_sequence()
        .and_then(|q| q.find_item(filmcraft_project::ClipId(avatar)))
        .and_then(|(_, c)| filmcraft_project::graphic::layer_indices(&c.effects).get(mouth as usize).copied())
        .ok_or("mouth effect missing")?;
    // The generated narration is FilmCraft's own mono float WAV. RMS drives a simple mouth,
    // not phoneme-accurate visemes. Validate its known header and cap reads.
    let wavpath = voice.get("path").and_then(Value::as_str).ok_or("voice path missing")?;
    let bytes = read_capped(Path::new(wavpath), 32_000_000)?;
    if bytes.get(0..4) != Some(b"RIFF") || bytes.get(36..40) != Some(b"data") {
        return Err("unexpected narration WAV layout".into());
    }
    let sample_rate = number(&voice, "sampleRate")?;
    let samples: Vec<f32> = bytes.get(44..).ok_or("WAV truncated")?.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    for f in (0..hold.0 / frame.0).step_by(2) {
        let n = usize::try_from(Tick(f * frame.0).to_units_floor(sample_rate as i64))?;
        let window = samples.get(n..n.saturating_add(sample_rate as usize / 12).min(samples.len())).unwrap_or(&[]);
        let energy = if window.is_empty() { 0.0 } else { (window.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / window.len() as f64).sqrt() };
        let height = (4.0 + energy * 120.0).clamp(4.0, 26.0);
        run(
            &mut s,
            "effects.setParam",
            json!({"clip":avatar,"effect":effect_index,"param":"size","value":[38,height],"time":(split+Tick(f * frame.0)).0,"keyframe":true}),
        )?;
    }
    run(
        &mut s,
        "graphics.newText",
        json!({"text":"LOCAL REACTION • reviewed script", "position":[20,32],"size":19,"time":split.0,"seconds":hold.seconds(),"track":2,"newClip":true}),
    )?;
    let project = out.join("reaction.fcproj");
    run(&mut s, "file.saveAs", json!({"path":project}))?;
    let inspect = run(&mut s, "sequence.inspect", json!({}))?;
    fs::write(out.join("timeline.json"), serde_json::to_vec_pretty(&inspect)?)?;
    fs::write(out.join("script.txt"), text)?;
    fs::write(
        out.join("provenance.json"),
        serde_json::to_vec_pretty(
            &json!({"source":source,"sourceTimestampTicks":split.0,"reactionStartTicks":split.0,"speechDurationTicks":speech.0,"holdDurationTicks":hold.0,"analysis":"none; fixed reviewed script","voice":voice_id,"avatar":"original procedural shape character; RMS mouth animation","cloudCalls":0}),
        )?,
    )?;
    let export = run(
        &mut s,
        "file.exportMedia",
        json!({"path":out.join("reaction.mp4"),"format":"h264","hardwareEncoding":"off","width":640,"height":360,"fps":24,"wait":true}),
    )?;
    if export.get("offlineMedia").and_then(Value::as_array).is_some_and(|x| !x.is_empty()) {
        return Err("export used offline media".into());
    }
    fs::write(out.join("export.json"), serde_json::to_vec_pretty(&export)?)?;
    let mut reopened = Session::default();
    run(&mut reopened, "file.open", json!({"path":project}))?;
    let after = run(&mut reopened, "sequence.inspect", json!({}))?;
    for key in ["video", "audio", "duration", "durationFrames"] {
        if inspect.get(key) != after.get(key) {
            return Err(format!("project reopen changed {key}").into());
        }
    }
    fs::write(out.join("reopened-timeline.json"), serde_json::to_vec_pretty(&after)?)?;
    eprintln!("Export and project reopen completed: {}", out.display());
    Ok(())
}

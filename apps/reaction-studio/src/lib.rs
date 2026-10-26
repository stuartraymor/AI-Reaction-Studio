#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented, clippy::unreachable))]
pub mod analysis;
pub mod avatar;
pub mod download;
pub mod pipeline;
pub mod providers;
pub mod render;
pub mod revision;
pub mod source;
pub mod style;
mod subtitles;

use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub const TICKS: i64 = filmcraft_time::TICKS_PER_SECOND;

#[derive(Clone, Default)]
pub struct Progress {
    pub cancel: Arc<AtomicBool>,
    pub messages: Arc<std::sync::Mutex<Vec<String>>>,
    pub echo: bool,
}
impl Progress {
    pub fn step(&self, text: &str) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err("Cancelled; saved analysis and assets are retained.".into());
        }
        if self.echo {
            eprintln!("{text}");
        }
        let mut messages = self.messages.lock().unwrap_or_else(|e| e.into_inner());
        if messages.len() >= 200 {
            messages.remove(0);
        }
        messages.push(text.into());
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub id: usize,
    pub start: i64,
    pub end: i64,
    pub text: String,
    pub kind: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Visual {
    pub id: usize,
    pub timestamp: i64,
    pub image: String,
    pub description: String,
    pub status: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Analysis {
    pub schema_version: u32,
    pub source: PathBuf,
    pub source_sha256: String,
    pub duration: i64,
    pub transcript: serde_json::Value,
    pub evidence: Vec<Evidence>,
    pub scene_cuts: Vec<i64>,
    pub visuals: Vec<Visual>,
    pub warnings: Vec<String>,
    pub models: serde_json::Value,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub evidence_id: usize,
    pub at: i64,
    pub source_quote: String,
    pub commentary: String,
    pub classification: String,
    pub verification: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct SourceClaim {
    pub evidence_id: usize,
    pub interpretation: String,
    pub kind: String,
    pub verification: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct ArgumentAnalysis {
    pub tentative_tone: String,
    pub tentative_rhetorical_techniques: Vec<String>,
    pub claims: Vec<SourceClaim>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub schema_version: u32,
    pub source_sha256: String,
    pub perspective: String,
    pub intensity: u8,
    #[serde(default)]
    pub custom_instructions: String,
    /// Reaction style id (`style.rs`); empty for scripts made before styles existed.
    #[serde(default)]
    pub style: String,
    pub review_required: bool,
    pub summary: String,
    pub segments: Vec<Segment>,
    pub argument_analysis: ArgumentAnalysis,
}
pub fn read_capped(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut b = Vec::new();
    fs::File::open(path)?.take(limit.saturating_add(1)).read_to_end(&mut b)?;
    if b.len() as u64 > limit {
        return Err(format!("{} exceeds {} bytes", path.display(), limit).into());
    }
    Ok(b)
}
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_slice(&read_capped(path, 16_000_000)?)?)
}
pub fn save_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().ok_or("output has no parent")?;
    fs::create_dir_all(parent)?;
    let temp = path.with_extension("json.pending");
    fs::write(&temp, serde_json::to_vec_pretty(value)?)?;
    // File::create does not touch any other file; failed serialization leaves the previous file.
    // Windows rename cannot replace an existing destination atomically through std alone.
    // Keep a recoverable backup before replacing these small, generated JSON documents.
    if path.exists() {
        fs::copy(path, path.with_extension("json.backup"))?;
    }
    fs::copy(&temp, path)?;
    Ok(())
}
pub fn data_dir() -> PathBuf {
    std::env::var_os("REACTION_STUDIO_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("AI Reaction Studio").join("Data"))
}
pub fn bootstrap() -> Result<Option<i32>> {
    if std::env::var("REACTION_STUDIO_BOOTSTRAPPED").as_deref() == Ok("1")
        && std::env::var_os("FILMCRAFT_DATA_DIR").map(PathBuf::from).as_ref() == Some(&data_dir())
    {
        return Ok(None);
    }
    // Set child-process environment through safe std APIs, rather than mutating Rust 2024's
    // process-wide environment after threads have started. Keep this app's settings separate.
    let code = std::process::Command::new(std::env::current_exe()?)
        .args(std::env::args_os().skip(1))
        .env("FILMCRAFT_DATA_DIR", data_dir())
        .env("REACTION_STUDIO_DATA_DIR", data_dir())
        .env("REACTION_STUDIO_BOOTSTRAPPED", "1")
        .status()?
        .code()
        .unwrap_or(1);
    Ok(Some(code))
}
pub fn exec(s: &mut filmcraft_engine::Session, id: &str, p: serde_json::Value) -> Result<serde_json::Value> {
    Ok(s.execute(id, p).map_err(|e| format!("{id}: {e}"))?)
}
pub fn id(v: &serde_json::Value, key: &str) -> Result<u64> {
    v.get(key).and_then(serde_json::Value::as_u64).ok_or_else(|| format!("missing {key}: {v}").into())
}
pub fn digest(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    let mut f = fs::File::open(path)?;
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(buf.get(..n).ok_or("invalid hash read")?);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn validate_script(analysis: &Analysis, script: &Script) -> Result<()> {
    if script.schema_version != 1
        || script.source_sha256 != analysis.source_sha256
        || script.intensity > 100
        || script.custom_instructions.len() > 4000
        || script.style.len() > 32
    {
        return Err("script schema, source fingerprint or intensity is invalid".into());
    }
    if script.segments.is_empty() || script.segments.len() > 8 {
        return Err("script must contain 1–8 reactions".into());
    }
    if script.argument_analysis.claims.len() > 8
        || script.argument_analysis.tentative_tone.len() > 1000
        || script.argument_analysis.tentative_rhetorical_techniques.len() > 8
    {
        return Err("argument analysis exceeds preview limits".into());
    }
    for claim in &script.argument_analysis.claims {
        if analysis.evidence.get(claim.evidence_id).filter(|e| e.id == claim.evidence_id).is_none()
            || claim.interpretation.len() > 2000
            || !["claim", "question", "conclusion"].contains(&claim.kind.as_str())
            || claim.verification != "unverified model interpretation of source"
        {
            return Err("argument analysis has invalid or unsupported evidence".into());
        }
    }
    let mut last = -1;
    for r in &script.segments {
        let e = analysis.evidence.get(r.evidence_id).filter(|e| e.id == r.evidence_id).ok_or("script references missing source evidence")?;
        if r.source_quote != e.text || r.at != e.end || r.at <= last || r.at <= 0 || r.at >= analysis.duration {
            return Err("reaction timestamps/quotes must match ordered source evidence exactly".into());
        }
        if r.commentary.trim().is_empty() || r.commentary.len() > 2000 {
            return Err("each commentary must contain 1–2000 UTF-8 bytes".into());
        }
        if r.classification != "opinion" || r.verification != "not independently verified" {
            return Err("model commentary must stay labelled as unverified opinion".into());
        }
        last = r.at;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn input() -> (Analysis, Script) {
        let a = Analysis {
            schema_version: 1,
            source: PathBuf::from("source.mp4"),
            source_sha256: "test-digest".into(),
            duration: 3 * TICKS,
            transcript: serde_json::Value::Null,
            evidence: vec![Evidence { id: 0, start: 0, end: TICKS, text: "An observed source statement.".into(), kind: "machine_transcript".into() }],
            scene_cuts: vec![],
            visuals: vec![],
            warnings: vec![],
            models: serde_json::Value::Null,
        };
        let s = Script {
            schema_version: 1,
            source_sha256: a.source_sha256.clone(),
            perspective: "Neutral".into(),
            intensity: 30,
            custom_instructions: String::new(),
            style: String::new(),
            review_required: true,
            summary: String::new(),
            segments: vec![Segment {
                evidence_id: 0,
                at: TICKS,
                source_quote: a.evidence[0].text.clone(),
                commentary: "That inference needs evidence.".into(),
                classification: "opinion".into(),
                verification: "not independently verified".into(),
            }],
            argument_analysis: ArgumentAnalysis { tentative_tone: "unknown".into(), tentative_rhetorical_techniques: vec![], claims: vec![] },
        };
        (a, s)
    }
    #[test]
    fn accepts_grounded_script() {
        let (a, s) = input();
        assert!(validate_script(&a, &s).is_ok());
    }
    #[test]
    fn rejects_invented_quote() {
        let (a, mut s) = input();
        s.segments[0].source_quote = "This was never said.".into();
        assert!(validate_script(&a, &s).is_err());
    }
    #[test]
    fn rejects_shifted_timestamp() {
        let (a, mut s) = input();
        s.segments[0].at += 1;
        assert!(validate_script(&a, &s).is_err());
    }
    #[test]
    fn rejects_another_source() {
        let (a, mut s) = input();
        s.source_sha256 = "other".into();
        assert!(validate_script(&a, &s).is_err());
    }
    #[test]
    fn rejects_unverified_claim_labelled_fact() {
        let (a, mut s) = input();
        s.segments[0].classification = "verified fact".into();
        assert!(validate_script(&a, &s).is_err());
    }
    #[test]
    fn rejects_missing_evidence() {
        let (a, mut s) = input();
        s.segments[0].evidence_id = usize::MAX;
        assert!(validate_script(&a, &s).is_err());
    }
    #[test]
    fn scripts_saved_before_styles_still_load() {
        let (_, s) = input();
        let mut v = serde_json::to_value(&s).unwrap();
        v.as_object_mut().unwrap().remove("style");
        let old: Script = serde_json::from_value(v).unwrap();
        assert_eq!(old.style, "");
    }
    #[test]
    fn rejects_oversized_style_id() {
        let (a, mut s) = input();
        s.style = "x".repeat(33);
        assert!(validate_script(&a, &s).is_err());
    }
    #[test]
    fn refuses_render_without_approval() {
        let (a, s) = input();
        assert!(render::render(&a, &s, Path::new("unused"), "kokoro-heart", "Violet", false, &Progress::default()).is_err());
    }
}

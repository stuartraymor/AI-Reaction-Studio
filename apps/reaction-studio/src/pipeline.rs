//! The one-button path: link or file → download → analyze → script in the chosen style → render.
//! Each stage saves its JSON, so a second style for the same video reuses the download and analysis.
use crate::source::{SourceInput, parse_source};
use crate::{Analysis, Progress, Result, Script, analysis, data_dir, digest, download, read_json, render, save_json, style};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct MakeOptions {
    pub style: String,
    pub extra: String,
    pub count: usize,
    pub voice: String,
    pub character: String,
    pub subtitles: bool,
    /// Render straight away. Without it the run stops at the draft so a person can review it.
    pub approve: bool,
}
impl Default for MakeOptions {
    fn default() -> Self {
        Self {
            style: style::DEFAULT_STYLE.into(),
            extra: String::new(),
            count: 3,
            voice: "kokoro-heart".into(),
            character: "Violet".into(),
            subtitles: true,
            approve: false,
        }
    }
}
#[derive(Debug)]
pub enum Made {
    NeedsReview { script: PathBuf },
    Rendered { folder: PathBuf },
}

pub const MAKE_USAGE: &str = "make LINK_OR_FILE --out PROJECT_DIR [--style hype|funny|skeptic|debate|explainer|chill] [--extra TEXT] [--count 1-4] [--voice ID] [--character Violet|Teal] [--no-subtitles] [--approve]";

/// Parses the arguments after `make`.
pub fn parse_make_args(args: &[String]) -> Result<(String, PathBuf, MakeOptions)> {
    let mut o = MakeOptions::default();
    let (mut source, mut out) = (None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().cloned().ok_or_else(|| format!("{name} needs a value; usage: {MAKE_USAGE}"));
        match a.as_str() {
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--style" => o.style = value("--style")?,
            "--extra" => o.extra = value("--extra")?,
            "--count" => o.count = value("--count")?.parse().map_err(|_| "--count must be 1-4")?,
            "--voice" => o.voice = value("--voice")?,
            "--character" => o.character = value("--character")?,
            "--no-subtitles" => o.subtitles = false,
            "--approve" => o.approve = true,
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}; usage: {MAKE_USAGE}").into()),
            _ if source.is_none() => source = Some(a.clone()),
            _ => return Err(format!("unexpected argument {a}; usage: {MAKE_USAGE}").into()),
        }
    }
    check_options(&o)?;
    Ok((source.ok_or(MAKE_USAGE)?, out.ok_or(MAKE_USAGE)?, o))
}
fn check_options(o: &MakeOptions) -> Result<()> {
    if style::find(&o.style).is_none() {
        let ids: Vec<&str> = style::STYLES.iter().map(|s| s.id).collect();
        return Err(format!("unknown style {}; choose one of {}", o.style, ids.join(", ")).into());
    }
    if !(1..=4).contains(&o.count) || o.extra.len() > 2000 {
        return Err("count must be 1-4 and --extra at most 2000 bytes".into());
    }
    Ok(())
}

/// The saved analysis in `folder`, if it belongs to exactly this (unchanged) video file.
pub fn reusable_analysis(folder: &Path, video: &Path) -> Option<Analysis> {
    let a: Analysis = read_json(&folder.join("analysis.json")).ok()?;
    let same_file = a.source.canonicalize().ok()? == video.canonicalize().ok()?;
    (same_file && digest(video).ok()? == a.source_sha256).then_some(a)
}

/// What the writer gets: (perspective, intensity, instructions), usually from `style::resolve`.
pub type Writer = (String, u8, String);
/// Download (if a link), analyze (or reuse the matching saved analysis) and draft a script in
/// `folder`. Saves `script.json` and `script-<style>.json`; the draft still needs review.
pub fn draft(source: &str, folder: &Path, style_id: &str, writer: &Writer, count: usize, p: &Progress) -> Result<(Analysis, Script, PathBuf)> {
    if style_id.is_empty() || style_id.len() > 32 || !style_id.bytes().all(|b| b.is_ascii_lowercase()) {
        return Err("invalid reaction style".into());
    }
    let input = parse_source(source)?;
    let video = match &input {
        SourceInput::File(path) if !path.is_file() => return Err(format!("video not found: {}", path.display()).into()),
        _ => download::fetch(&input, &data_dir().join("downloads"), p)?,
    };
    let a = match reusable_analysis(folder, &video) {
        Some(a) => {
            p.step("Reusing the saved analysis of this video")?;
            a
        }
        None => analysis::analyze(&video, folder, p)?,
    };
    let (perspective, intensity, instructions) = writer;
    let mut script: Script = analysis::generate(&a, perspective, *intensity, instructions, count, p)?;
    script.style = style_id.into();
    let script_path = folder.join(format!("script-{style_id}.json"));
    save_json(&script_path, &script)?;
    save_json(&folder.join("script.json"), &script)?;
    Ok((a, script, script_path))
}
/// Draft (and, with `approve`, render) a reaction in `folder` using a preset style.
pub fn make_reaction(source: &str, folder: &Path, o: &MakeOptions, p: &Progress) -> Result<Made> {
    check_options(o)?;
    let writer = style::resolve(&o.style, "", 0, "", &o.extra);
    let (a, script, script_path) = draft(source, folder, &o.style, &writer, o.count, p)?;
    if !o.approve {
        return Ok(Made::NeedsReview { script: script_path });
    }
    let out = folder.join(format!("render-{}-{}", o.style, stamp()));
    render::render_with_subtitles(&a, &script, &out, &o.voice, &o.character, true, o.subtitles, p)?;
    Ok(Made::Rendered { folder: out })
}
/// `reaction-pipeline make …`: a reaction draft (or video, with `--approve`) in `--out`.
pub fn make_reaction_from_args(args: &[String], p: &Progress) -> Result<Made> {
    let (source, folder, o) = parse_make_args(args)?;
    make_reaction(&source, &folder, &o, p)
}
pub fn stamp() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::input;
    fn args(s: &[&str]) -> Vec<String> {
        s.iter().map(|a| a.to_string()).collect()
    }
    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("reaction-pipe-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    #[test]
    fn make_args_parse_with_defaults() {
        let (src, out, o) = parse_make_args(&args(&["https://youtu.be/dQw4w9WgXcQ", "--out", "proj"])).unwrap();
        assert_eq!((src.as_str(), out), ("https://youtu.be/dQw4w9WgXcQ", PathBuf::from("proj")));
        assert_eq!(o, MakeOptions::default());
        let (_, _, o) =
            parse_make_args(&args(&["v.mp4", "--style", "funny", "--count", "2", "--extra", "be kind", "--no-subtitles", "--approve", "--out", "p"])).unwrap();
        assert_eq!((o.style.as_str(), o.count, o.extra.as_str(), o.subtitles, o.approve), ("funny", 2, "be kind", false, true));
    }
    #[test]
    fn make_args_reject_mistakes() {
        for bad in [
            &["v.mp4"][..],
            &["--out", "p"],
            &["v.mp4", "--out"],
            &["v.mp4", "--out", "p", "--style", "angry"],
            &["v.mp4", "--out", "p", "--style", "custom"],
            &["v.mp4", "--out", "p", "--count", "9"],
            &["v.mp4", "--out", "p", "--count", "x"],
            &["v.mp4", "--out", "p", "--colour"],
            &["v.mp4", "w.mp4", "--out", "p"],
        ] {
            assert!(parse_make_args(&args(bad)).is_err(), "{bad:?}");
        }
    }
    #[test]
    fn analysis_is_reused_only_for_the_same_unchanged_video() {
        let d = temp("reuse");
        let video = d.join("clip.mp4");
        std::fs::write(&video, b"video bytes").unwrap();
        assert!(reusable_analysis(&d, &video).is_none(), "nothing saved yet");
        let (mut a, _) = input();
        a.source = video.clone();
        a.source_sha256 = digest(&video).unwrap();
        save_json(&d.join("analysis.json"), &a).unwrap();
        assert!(reusable_analysis(&d, &video).is_some());
        let other = d.join("other.mp4");
        std::fs::write(&other, b"video bytes").unwrap();
        assert!(reusable_analysis(&d, &other).is_none(), "different file");
        std::fs::write(&video, b"edited video").unwrap();
        assert!(reusable_analysis(&d, &video).is_none(), "changed file");
        std::fs::write(d.join("analysis.json"), b"{not json").unwrap();
        assert!(reusable_analysis(&d, &video).is_none(), "corrupt report");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn make_fails_fast_before_any_slow_work() {
        let d = temp("fast");
        let p = Progress::default();
        let bad_style = MakeOptions { style: "nope".into(), ..MakeOptions::default() };
        assert!(make_reaction("x.mp4", &d, &bad_style, &p).is_err());
        assert!(make_reaction("https://evil.example/watch?v=dQw4w9WgXcQ", &d, &MakeOptions::default(), &p).is_err());
        let missing = make_reaction(&d.join("missing.mp4").to_string_lossy(), &d, &MakeOptions::default(), &p).unwrap_err();
        assert!(missing.to_string().contains("not found"));
        let writer: Writer = ("Neutral".into(), 10, String::new());
        for bad in ["", "Hype", "../x", &"a".repeat(40)] {
            assert!(draft("x.mp4", &d, bad, &writer, 1, &p).is_err(), "{bad}");
        }
        assert!(p.messages.lock().unwrap().is_empty(), "no stage started");
        let _ = std::fs::remove_dir_all(&d);
    }
}

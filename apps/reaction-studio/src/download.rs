//! Fetches a YouTube video with an external `yt-dlp` the user installed. It is never shipped or
//! linked: we only run it as a separate program, like ffmpeg in tests. Only the validated 11-char id
//! reaches its command line (as a rebuilt canonical URL after `--`), and user config files are ignored.
use crate::source::SourceInput;
use crate::{Progress, Result};
use std::{
    ffi::{OsStr, OsString},
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

/// Matches the analyzer's 15-minute limit; checked by yt-dlp before downloading.
pub const MAX_SECONDS: u32 = 900;
pub const MAX_BYTES: u64 = 2_000_000_000;
const TIMEOUT: Duration = Duration::from_secs(60 * 60);
const PROGRESS_TAG: &str = "RSDL";
pub const RIGHTS_NOTICE: &str = "Only download videos you own or have permission to use, and follow YouTube's Terms of Service.";
const NOT_FOUND: &str = "yt-dlp was not found. Install it from https://github.com/yt-dlp/yt-dlp (or set REACTION_STUDIO_YTDLP to its path), \
     or download the video yourself and choose the file instead.";

/// `REACTION_STUDIO_YTDLP` wins; otherwise the first `yt-dlp` on PATH.
pub fn downloader_path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("REACTION_STUDIO_YTDLP").filter(|p| !p.is_empty()) {
        let p = PathBuf::from(p);
        return if p.is_file() { Ok(p) } else { Err(format!("REACTION_STUDIO_YTDLP points to a missing file: {}", p.display()).into()) };
    }
    find_on_path(std::env::var_os("PATH").as_deref()).ok_or_else(|| NOT_FOUND.into())
}
fn find_on_path(path: Option<&OsStr>) -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["yt-dlp.exe", "yt-dlp"] } else { &["yt-dlp"] };
    std::env::split_paths(path?).take(512).flat_map(|dir| names.iter().map(move |n| dir.join(n))).find(|p| p.is_file())
}

/// Picks a single progressive H.264/AAC MP4 (no ffmpeg merge needed), at most 720p.
pub fn build_args(id: &str, part_dir: &Path) -> Result<Vec<OsString>> {
    let url = SourceInput::YouTube { id: id.into() }.canonical_url().ok_or("not a YouTube source")?;
    if crate::source::parse_source(&url)? != (SourceInput::YouTube { id: id.into() }) {
        return Err("invalid YouTube video id".into());
    }
    let mut args: Vec<OsString> = [
        "--ignore-config",
        "--no-playlist",
        "--no-mtime",
        "--continue",
        "--newline",
        "--no-colors",
        "--restrict-filenames",
        "--progress-template",
        &format!("download:{PROGRESS_TAG} %(progress.downloaded_bytes)s %(progress.total_bytes)s %(progress.total_bytes_estimate)s"),
        "--max-filesize",
        &MAX_BYTES.to_string(),
        "--match-filter",
        &format!("duration<={MAX_SECONDS} & !is_live"),
        "-f",
        "b[ext=mp4][vcodec^=avc1][acodec!=none][height<=720]/b[ext=mp4][acodec!=none][height<=720]",
        "-o",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    args.push(part_dir.join(format!("{id}.%(ext)s")).into_os_string());
    args.push("--".into());
    args.push(url.into());
    Ok(args)
}

/// `RSDL <downloaded> <total> <estimate>`; yt-dlp prints `NA` for unknown fields.
pub fn parse_progress(line: &str) -> Option<(u64, Option<u64>)> {
    let mut it = line.trim().split_whitespace();
    if it.next()? != PROGRESS_TAG {
        return None;
    }
    let done = it.next()?.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0)? as u64;
    let total = it.filter_map(|v| v.parse::<f64>().ok()).find(|v| v.is_finite() && *v > 0.0).map(|v| v as u64);
    Some((done, total))
}

/// Turns yt-dlp's output into one plain sentence for the user.
pub fn classify_error(output: &str) -> String {
    let o = output.to_ascii_lowercase();
    let known: &[(&[&str], &str)] = &[
        (&["private video"], "This video is private."),
        (&["confirm your age", "age-restricted", "inappropriate for some users"], "This video is age-restricted and can't be downloaded without signing in."),
        (&["members-only", "join this channel"], "This video is for channel members only."),
        (&["not a bot", "http error 429", "too many requests"], "YouTube is temporarily blocking downloads from this computer. Try again later."),
        (&["does not pass filter", "is live", "live event"], "This video is longer than 15 minutes or is a live stream."),
        (&["larger than max-filesize"], "This video is too large (over 2 GB)."),
        (&["requested format is not available", "no video formats"], "No MP4 version of this video is available."),
        (&["video unavailable", "has been removed", "not available in your country", "copyright"], "This video is unavailable."),
        (
            &["getaddrinfo", "failed to resolve", "unable to download webpage", "timed out", "connection"],
            "Couldn't reach YouTube. Check your internet connection.",
        ),
    ];
    known.iter().find(|(needles, _)| needles.iter().any(|n| o.contains(n))).map(|(_, msg)| (*msg).to_string()).unwrap_or_else(|| {
        let last = output.lines().rev().find(|l| l.contains("ERROR")).unwrap_or("no error details").trim();
        format!("Download failed: {}", last.chars().take(300).collect::<String>())
    })
}

/// Moves a finished `<id>.mp4` out of the partial folder only after checking it is a plausible MP4.
pub fn finalize(part_dir: &Path, id: &str, dest: &Path) -> Result<PathBuf> {
    let file = part_dir.join(format!("{id}.mp4"));
    check_mp4(&file).map_err(|e| format!("download did not produce a usable video: {e}"))?;
    fs::rename(&file, dest)?;
    let _ = fs::remove_dir_all(part_dir);
    Ok(dest.to_path_buf())
}
fn check_mp4(path: &Path) -> Result<()> {
    let len = fs::metadata(path).map_err(|_| "no video file")?.len();
    if len < 12 || len > MAX_BYTES {
        return Err("video file is empty or too large".into());
    }
    let mut head = [0u8; 12];
    fs::File::open(path)?.read_exact(&mut head)?;
    if head.get(4..8) != Some(b"ftyp") {
        return Err("file is not an MP4".into());
    }
    Ok(())
}

/// Downloads into `dir/<id>.mp4` (reused if already there). Partial data stays in `dir/<id>.partial`
/// so a cancelled or failed download resumes next time. Local files pass through unchanged.
pub fn fetch(source: &SourceInput, dir: &Path, p: &Progress) -> Result<PathBuf> {
    let id = match source {
        SourceInput::File(path) => return Ok(path.clone()),
        SourceInput::YouTube { id } => id,
    };
    let dest = dir.join(format!("{id}.mp4"));
    if check_mp4(&dest).is_ok() {
        p.step("Using the previously downloaded video")?;
        return Ok(dest);
    }
    let exe = downloader_path()?;
    let part_dir = dir.join(format!("{id}.partial"));
    fs::create_dir_all(&part_dir)?;
    p.step(RIGHTS_NOTICE)?;
    p.step("Downloading the video with yt-dlp")?;
    let mut cmd = Command::new(exe);
    cmd.args(build_args(id, &part_dir)?);
    let out = run(cmd, TIMEOUT, p)?;
    if !out.success {
        return Err(classify_error(&out.output).into());
    }
    finalize(&part_dir, id, &dest).map_err(|e| {
        // yt-dlp exits 0 when a match filter or size limit skips the video.
        if out.output.to_ascii_lowercase().contains("skipping") || out.output.contains("max-filesize") { classify_error(&out.output).into() } else { e }
    })
}

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        // The Windows yt-dlp.exe is a self-extracting launcher with a child process; stop the tree.
        #[cfg(windows)]
        if let Ok(None) = self.0.try_wait() {
            use std::os::windows::process::CommandExt;
            let _ = Command::new("taskkill")
                .args(["/T", "/F", "/PID", &self.0.id().to_string()])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x08000000)
                .status();
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub struct RunOutput {
    pub success: bool,
    /// Non-progress stdout and stderr lines, capped, for error messages.
    pub output: String,
}
enum Line {
    Text(String),
    Done,
}
/// Runs the downloader, reporting progress, honouring cancel and a hard timeout.
pub fn run(mut cmd: Command, timeout: Duration, p: &Progress) -> Result<RunOutput> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let mut child = Running(cmd.spawn().map_err(|e| format!("could not start the downloader: {e}"))?);
    let (tx, rx) = mpsc::channel();
    let pipes: [Option<Box<dyn Read + Send>>; 2] =
        [child.0.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>), child.0.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>)];
    let mut open = 0;
    for pipe in pipes.into_iter().flatten() {
        let tx = tx.clone();
        open += 1;
        std::thread::spawn(move || {
            for line in BufReader::new(pipe).lines().map_while(std::io::Result::ok) {
                if tx.send(Line::Text(line)).is_err() {
                    return;
                }
            }
            let _ = tx.send(Line::Done);
        });
    }
    drop(tx);
    let start = Instant::now();
    let mut output = String::new();
    let mut last_mb = u64::MAX;
    loop {
        if p.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Cancelled; the partial download is kept and will resume next time.".into());
        }
        if start.elapsed() > timeout {
            return Err("the download took too long and was stopped; try again".into());
        }
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Line::Text(line)) => {
                if let Some((done, total)) = parse_progress(&line) {
                    let mb = done / 1_000_000;
                    if mb != last_mb {
                        last_mb = mb;
                        let total = total.map(|t| format!(" / {}", t / 1_000_000)).unwrap_or_default();
                        p.step(&format!("Downloading video: {mb}{total} MB"))?;
                    }
                } else if output.len() < 64_000 {
                    output.push_str(&line);
                    output.push('\n');
                }
            }
            Ok(Line::Done) => open -= 1,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => open = 0,
        }
        if open == 0 {
            if let Some(status) = child.0.try_wait()? {
                return Ok(RunOutput { success: status.success(), output });
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::AtomicBool};
    const ID: &str = "dQw4w9WgXcQ";

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("reaction-dl-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn fake_mp4(path: &Path) {
        let mut b = vec![0, 0, 0, 24];
        b.extend_from_slice(b"ftypisom");
        b.resize(64, 0);
        fs::write(path, b).unwrap();
    }
    fn shell(script: &str) -> Command {
        if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", script]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", script]);
            c
        }
    }

    #[test]
    fn args_ignore_config_end_options_and_carry_only_the_canonical_url() {
        let args = build_args(ID, Path::new("dl")).unwrap();
        let s: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(s.contains(&"--ignore-config".into()) && s.contains(&"--no-playlist".into()));
        assert_eq!(s[s.len() - 2], "--");
        assert_eq!(s[s.len() - 1], format!("https://www.youtube.com/watch?v={ID}"));
        assert!(s.iter().any(|a| a == &format!("duration<={MAX_SECONDS} & !is_live")));
        assert!(s.iter().any(|a| a == &MAX_BYTES.to_string()));
        assert!(!s.iter().any(|a| a.contains("--exec") || a.contains("--cookies")));
    }
    #[test]
    fn args_reject_ids_that_could_smuggle_options() {
        for bad in ["--exec=calc", "abc", "dQw4w9WgXc&", "dQw4w9WgXcQ --", ""] {
            assert!(build_args(bad, Path::new("dl")).is_err(), "{bad}");
        }
    }
    #[test]
    fn progress_lines_parse_and_others_are_ignored() {
        assert_eq!(parse_progress("RSDL 1048576 2097152 NA"), Some((1048576, Some(2097152))));
        assert_eq!(parse_progress("RSDL 500 NA 9000.5"), Some((500, Some(9000))));
        assert_eq!(parse_progress("RSDL 500 NA NA"), Some((500, None)));
        for junk in ["", "RSDL", "RSDL NA NA NA", "RSDL -5 10 10", "RSDL inf 1 1", "[download] 50%", "XRSDL 1 2 3"] {
            assert_eq!(parse_progress(junk), None, "{junk}");
        }
    }
    #[test]
    fn errors_become_plain_sentences() {
        let cases = [
            ("ERROR: [youtube] x: Private video. Sign in if you've been granted access", "private"),
            ("ERROR: [youtube] x: Sign in to confirm your age.", "age-restricted"),
            ("ERROR: [youtube] x: Join this channel to get access to members-only content", "members"),
            ("ERROR: [youtube] x: Sign in to confirm you're not a bot", "blocking"),
            ("[download] Long video does not pass filter (duration<=900), skipping ..", "15 minutes"),
            ("ERROR: [youtube] x: Video unavailable", "unavailable"),
            ("ERROR: Unable to download webpage: <urlopen error [Errno 11001] getaddrinfo failed>", "internet"),
        ];
        for (out, want) in cases {
            assert!(classify_error(out).contains(want), "{out} -> {}", classify_error(out));
        }
        let other = classify_error("WARNING: x\nERROR: something new\n");
        assert!(other.contains("something new"));
        assert!(classify_error(&"é".repeat(10_000)).len() < 1000);
    }
    #[test]
    fn finalize_moves_only_real_mp4s_and_cleans_up() {
        let d = temp("finalize");
        let part = d.join(format!("{ID}.partial"));
        fs::create_dir_all(&part).unwrap();
        let dest = d.join(format!("{ID}.mp4"));
        assert!(finalize(&part, ID, &dest).is_err(), "missing file");
        fs::write(part.join(format!("{ID}.mp4")), b"<html>not a video</html>").unwrap();
        assert!(finalize(&part, ID, &dest).is_err(), "html page");
        assert!(!dest.exists());
        fake_mp4(&part.join(format!("{ID}.mp4")));
        assert_eq!(finalize(&part, ID, &dest).unwrap(), dest);
        assert!(dest.exists() && !part.exists());
        let _ = fs::remove_dir_all(&d);
    }
    #[test]
    fn local_files_and_finished_downloads_skip_the_downloader() {
        let d = temp("reuse");
        let file = SourceInput::File(PathBuf::from("my clip.mp4"));
        assert_eq!(fetch(&file, &d, &Progress::default()).unwrap(), PathBuf::from("my clip.mp4"));
        fake_mp4(&d.join(format!("{ID}.mp4")));
        assert_eq!(fetch(&SourceInput::YouTube { id: ID.into() }, &d, &Progress::default()).unwrap(), d.join(format!("{ID}.mp4")));
        let _ = fs::remove_dir_all(&d);
    }
    #[test]
    fn path_search_finds_yt_dlp_or_nothing() {
        let d = temp("path");
        assert_eq!(find_on_path(Some(d.as_os_str())), None);
        assert_eq!(find_on_path(None), None);
        let exe = d.join(if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" });
        fs::write(&exe, b"").unwrap();
        let joined = std::env::join_paths([d.join("missing"), d.clone()]).unwrap();
        assert_eq!(find_on_path(Some(&joined)), Some(exe));
        let _ = fs::remove_dir_all(&d);
    }
    #[test]
    fn run_reports_progress_and_collects_errors() {
        let p = Progress::default();
        let out = run(shell("echo RSDL 3000000 6000000 NA&& echo ERROR: Video unavailable 1>&2&& exit 1"), Duration::from_secs(30), &p).unwrap();
        assert!(!out.success);
        assert!(out.output.contains("Video unavailable") && !out.output.contains("RSDL"));
        let messages = p.messages.lock().unwrap();
        assert!(messages.iter().any(|m| m.contains("3 / 6 MB")), "{messages:?}");
    }
    #[test]
    fn cancel_stops_a_running_download_quickly() {
        let p = Progress { cancel: Arc::new(AtomicBool::new(false)), ..Progress::default() };
        let cancel = p.cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        let start = Instant::now();
        let slow = if cfg!(windows) { shell("ping -n 30 127.0.0.1 >NUL") } else { shell("sleep 30") };
        let err = run(slow, Duration::from_secs(60), &p).err().unwrap();
        assert!(err.to_string().contains("Cancelled"));
        assert!(start.elapsed() < Duration::from_secs(10));
    }
    #[test]
    fn timeout_stops_a_hung_download() {
        let slow = if cfg!(windows) { shell("ping -n 30 127.0.0.1 >NUL") } else { shell("sleep 30") };
        let start = Instant::now();
        let err = run(slow, Duration::from_millis(300), &Progress::default()).err().unwrap();
        assert!(err.to_string().contains("too long"));
        assert!(start.elapsed() < Duration::from_secs(10));
    }
    #[test]
    fn missing_downloader_is_a_clear_error() {
        let err = run(Command::new("definitely-not-yt-dlp-xyz"), Duration::from_secs(5), &Progress::default()).err().unwrap();
        assert!(err.to_string().contains("could not start"));
        assert!(NOT_FOUND.contains("choose the file"));
    }
}

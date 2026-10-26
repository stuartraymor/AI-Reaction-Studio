//! Parses the single "video" box: a local file path or a YouTube link. No network access here;
//! downloading is a separate, cancellable step that consumes a `SourceInput::YouTube`.
use crate::Result;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceInput {
    File(PathBuf),
    YouTube { id: String },
}
impl SourceInput {
    /// Canonical watch URL, rebuilt from the validated id so nothing else from the input is trusted.
    pub fn canonical_url(&self) -> Option<String> {
        match self {
            SourceInput::YouTube { id } => Some(format!("https://www.youtube.com/watch?v={id}")),
            SourceInput::File(_) => None,
        }
    }
}
fn valid_id(id: &str) -> bool {
    id.len() == 11 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|kv| kv.strip_prefix(key)?.strip_prefix('='))
}
pub fn parse_source(input: &str) -> Result<SourceInput> {
    let input = input.trim().trim_matches('"');
    if input.is_empty() || input.len() > 2048 || input.chars().any(char::is_control) {
        return Err("enter a video file or a YouTube link".into());
    }
    let lower = input.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        if lower.contains("://") {
            return Err("only http(s) YouTube links are supported".into());
        }
        return Ok(SourceInput::File(PathBuf::from(input)));
    }
    let rest = input.split_once("://").map(|(_, r)| r).unwrap_or_default();
    let (authority, path_query) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.contains('@') {
        return Err("links containing credentials are not accepted".into());
    }
    let host = authority.split(':').next().unwrap_or_default().to_ascii_lowercase();
    let path_query = path_query.split('#').next().unwrap_or_default();
    let (path, query) = path_query.split_once('?').unwrap_or((path_query, ""));
    let mut parts = path.split('/').filter(|p| !p.is_empty());
    let id = match host.as_str() {
        "youtu.be" => parts.next(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com" => match parts.next() {
            Some("watch") => query_param(query, "v"),
            Some("shorts" | "embed" | "live" | "v") => parts.next(),
            _ => None,
        },
        _ => return Err("only youtube.com and youtu.be links are supported".into()),
    };
    match id {
        Some(id) if valid_id(id) => Ok(SourceInput::YouTube { id: id.into() }),
        _ if query_param(query, "list").is_some() => Err("playlists are not supported; paste a single video link".into()),
        _ => Err("could not find a video id in that YouTube link".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "dQw4w9WgXcQ";
    #[test]
    fn accepts_common_youtube_forms() {
        for url in [
            format!("https://www.youtube.com/watch?v={ID}"),
            format!("https://youtube.com/watch?feature=share&v={ID}&t=42s"),
            format!("https://m.youtube.com/watch?v={ID}"),
            format!("https://youtu.be/{ID}?si=abc"),
            format!("https://www.youtube.com/shorts/{ID}"),
            format!("https://www.youtube.com/embed/{ID}"),
            format!("https://www.youtube.com/live/{ID}?feature=share"),
            format!("  HTTPS://WWW.YOUTUBE.COM/watch?v={ID}#frag  "),
        ] {
            assert_eq!(parse_source(&url).unwrap(), SourceInput::YouTube { id: ID.into() }, "{url}");
        }
    }
    #[test]
    fn canonical_url_is_rebuilt_from_id_only() {
        let s = parse_source(&format!("https://youtu.be/{ID}?utm_source=x&evil=;rm")).unwrap();
        assert_eq!(s.canonical_url().unwrap(), format!("https://www.youtube.com/watch?v={ID}"));
    }
    #[test]
    fn rejects_lookalike_and_foreign_hosts() {
        for url in [
            format!("https://youtube.com.evil.example/watch?v={ID}"),
            format!("https://evil.example/youtube.com/watch?v={ID}"),
            format!("https://notyoutube.com/watch?v={ID}"),
            format!("https://www.youtube.com@evil.example/watch?v={ID}"),
            format!("https://vimeo.com/{ID}"),
        ] {
            assert!(parse_source(&url).is_err(), "{url}");
        }
    }
    #[test]
    fn rejects_bad_ids_playlists_and_schemes() {
        assert!(parse_source("https://www.youtube.com/watch?v=short").is_err());
        assert!(parse_source(&format!("https://www.youtube.com/watch?v={ID}x")).is_err());
        assert!(parse_source("https://www.youtube.com/watch?v=abc$def;rm-").is_err());
        assert!(parse_source("https://www.youtube.com/").is_err());
        assert!(parse_source("https://www.youtube.com/playlist?list=PL1234567890").unwrap_err().to_string().contains("playlist"));
        assert!(parse_source(&format!("ftp://youtube.com/watch?v={ID}")).is_err());
        assert!(parse_source(&format!("javascript://youtu.be/{ID}")).is_err());
    }
    #[test]
    fn plain_paths_are_files_and_hostile_input_is_rejected() {
        assert_eq!(parse_source(r#""C:\Videos\my clip.mp4""#).unwrap(), SourceInput::File(PathBuf::from(r"C:\Videos\my clip.mp4")));
        assert!(parse_source("").is_err());
        assert!(parse_source("   ").is_err());
        assert!(parse_source("clip.mp4\nrm -rf").is_err());
        assert!(parse_source(&"a".repeat(5000)).is_err());
    }
    #[test]
    fn never_panics_on_odd_input() {
        for s in [
            "https://",
            "https:///",
            "http://:",
            "https://youtu.be/",
            "https://youtu.be//",
            "https://www.youtube.com/watch?v",
            "https://www.youtube.com/watch?v=",
            "é://ü",
            "https://www.youtube.com/shorts/",
        ] {
            let _ = parse_source(s);
        }
    }
}

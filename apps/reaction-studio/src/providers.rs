use crate::{Progress, Result, data_dir, digest};
use base64::Engine;
use filmcraft_speech::models::ModelFile;
use serde_json::{Value, json};
use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub const VISION_FILES: &[ModelFile] = &[
    ModelFile {
        name: "SmolVLM2-500M-Video-Instruct-Q8_0.gguf",
        url: "https://huggingface.co/ggml-org/SmolVLM2-500M-Video-Instruct-GGUF/resolve/ccd7aae53bcb1997355c2f094959e72b3642ce17/SmolVLM2-500M-Video-Instruct-Q8_0.gguf",
        sha256: "6f67b8036b2469fcd71728702720c6b51aebd759b78137a8120733b4d66438bc",
        size: 436808704,
    },
    ModelFile {
        name: "mmproj-SmolVLM2-500M-Video-Instruct-Q8_0.gguf",
        url: "https://huggingface.co/ggml-org/SmolVLM2-500M-Video-Instruct-GGUF/resolve/ccd7aae53bcb1997355c2f094959e72b3642ce17/mmproj-SmolVLM2-500M-Video-Instruct-Q8_0.gguf",
        sha256: "921dc7e259f308e5b027111fa185efcbf33db13f6e35749ddf7f5cdb60ef520b",
        size: 108785184,
    },
];
pub const WRITER_FILES: &[ModelFile] = &[ModelFile {
    name: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
    url: "https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/resolve/a06e946bb6b655725eafa393f4a9745d460374c9/Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
    sha256: "3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597",
    size: 2497281120,
}];
pub fn setup(p: &Progress) -> Result<()> {
    let speech = filmcraft_speech::models::find("whisper-tiny").ok_or("Whisper tiny is missing from the catalog")?;
    let voice_files: Vec<ModelFile> =
        filmcraft_tts::catalog::FILES.iter().map(|f| ModelFile { name: f.name, url: f.url, sha256: f.sha256, size: f.size }).collect();
    for (group, files) in [("whisper-tiny", speech.files), ("kokoro-82m", voice_files.as_slice()), ("vision", VISION_FILES), ("writer", WRITER_FILES)] {
        let dir = data_dir().join("models").join(group);
        fs::create_dir_all(&dir)?;
        for f in files {
            let path = dir.join(f.name);
            if path.exists() && (fs::metadata(&path)?.len() != f.size || digest(&path)? != f.sha256) {
                let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
                fs::rename(&path, path.with_extension(format!("corrupt-{stamp}")))?;
            }
        }
        let license = match group {
            "whisper-tiny" => "MIT / Apache-2.0",
            "kokoro-82m" => "Apache-2.0; CMUdict BSD",
            _ => "Apache-2.0",
        };
        p.step(&format!("Downloading/checking {group} model ({license})"))?;
        filmcraft_speech::models::download_files(&dir, files, &mut |done, total, _file| {
            if total > 0 {
                let _ = p.step(&format!("{group}: {} / {} MB", done / 1_000_000, total / 1_000_000));
            }
            !p.cancel.load(std::sync::atomic::Ordering::Relaxed)
        })?;
    }
    p.step("Local models ready")?;
    Ok(())
}

/// Model groups whose files are absent or the wrong size under `models_dir`. A quick check for the
/// first-run screen; `LocalModel::start` still verifies hashes before use.
pub fn missing_models(models_dir: &Path) -> Vec<&'static str> {
    let speech = filmcraft_speech::models::find("whisper-tiny").map(|m| m.files.iter().map(|f| (f.name, f.size)).collect::<Vec<_>>()).unwrap_or_default();
    let voices: Vec<(&str, u64)> = filmcraft_tts::catalog::FILES.iter().map(|f| (f.name, f.size)).collect();
    let vision: Vec<(&str, u64)> = VISION_FILES.iter().map(|f| (f.name, f.size)).collect();
    let writer: Vec<(&str, u64)> = WRITER_FILES.iter().map(|f| (f.name, f.size)).collect();
    [("whisper-tiny", speech), ("kokoro-82m", voices), ("vision", vision), ("writer", writer)]
        .into_iter()
        .filter(|(group, files)| {
            files.is_empty() || files.iter().any(|(name, size)| fs::metadata(models_dir.join(group).join(name)).map(|m| m.len()).ok() != Some(*size))
        })
        .map(|(group, _)| group)
        .collect()
}

pub trait ChatProvider {
    fn chat(&self, messages: Value, schema: Option<Value>, max_tokens: usize) -> Result<String>;
}
pub struct LocalModel {
    child: Child,
    pub url: String,
    agent: ureq::Agent,
}
impl Drop for LocalModel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
pub fn runtime_path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("REACTION_LLAMA_SERVER") {
        return Ok(PathBuf::from(p));
    }
    Ok(std::env::current_exe()?.parent().ok_or("executable has no parent")?.join("runtime").join("llama-server.exe"))
}
impl LocalModel {
    pub fn start(vision: bool, p: &Progress) -> Result<Self> {
        let dir = data_dir().join("models").join(if vision { "vision" } else { "writer" });
        let files = if vision { VISION_FILES } else { WRITER_FILES };
        for f in files {
            let path = dir.join(f.name);
            if !path.exists() || fs::metadata(&path)?.len() != f.size || digest(&path)? != f.sha256 {
                return Err(format!("missing or corrupt model {}; use Download local models", f.name).into());
            }
        }
        let model = files.first().ok_or("empty model catalog")?;
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        let runtime = runtime_path()?;
        if !runtime.is_file() {
            return Err(format!("llama.cpp runtime missing: {}", runtime.display()).into());
        }
        fs::create_dir_all(data_dir().join("Logs"))?;
        let log = fs::File::create(data_dir().join("Logs").join(if vision { "vision-server.log" } else { "writer-server.log" }))?;
        let mut cmd = Command::new(runtime);
        cmd.args(["-m"]).arg(dir.join(model.name)).args([
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--ctx-size",
            "4096",
            "--parallel",
            "1",
            "--threads",
            "4",
            "--n-gpu-layers",
            "0",
            "--alias",
            "reaction-local",
            "--no-webui",
        ]);
        if vision {
            let projector = files.get(1).ok_or("vision projector missing")?;
            cmd.arg("--mmproj").arg(dir.join(projector.name));
        }
        cmd.stdin(Stdio::null()).stdout(Stdio::from(log.try_clone()?)).stderr(Stdio::from(log));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let child = cmd.spawn()?;
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(180))).build().into();
        let mut server = Self { child, url: format!("http://127.0.0.1:{port}"), agent };
        let start = Instant::now();
        let health: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(2))).build().into();
        loop {
            p.step(if vision { "Loading local vision model" } else { "Loading local commentary model" })?;
            if let Some(status) = server.child.try_wait()? {
                return Err(format!("local model server exited {status}; see Data/Logs").into());
            }
            if health.get(format!("{}/health", server.url)).call().is_ok() {
                break;
            }
            if start.elapsed() > Duration::from_secs(60) {
                return Err("local model startup timed out; see Data/Logs".into());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(server)
    }
    pub fn describe(&self, path: &Path) -> Result<String> {
        let image = image::open(path)?.thumbnail(512, 512).to_rgb8();
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png)?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes.into_inner());
        self.chat(json!([{"role":"user","content":[{"type":"image_url","image_url":{"url":format!("data:image/png;base64,{encoded}")}},{"type":"text","text":"Describe only the visible scene in two short sentences. Mention the main objects, colors and setting. Do not infer speech, identity, motive or facts outside this image. If unclear, say unclear."}]}]),None,120)
    }
}
impl ChatProvider for LocalModel {
    fn chat(&self, messages: Value, schema: Option<Value>, max_tokens: usize) -> Result<String> {
        let mut body = json!({"model":"reaction-local","messages":messages,"temperature":0.1,"seed":42,"max_tokens":max_tokens.min(1600),"stream":false});
        if let Some(schema) = schema {
            body["response_format"] = json!({"type":"json_object","schema":schema});
        }
        let mut response = self.agent.post(format!("{}/v1/chat/completions", self.url)).send_json(body)?;
        let result: Value = response.body_mut().with_config().limit(2_000_000).read_json()?;
        if result.pointer("/choices/0/finish_reason").and_then(Value::as_str) == Some("length") {
            return Err("model response exceeded token limit; reduce requested reactions".into());
        }
        let text = result.pointer("/choices/0/message/content").and_then(Value::as_str).ok_or("local model returned no text")?.trim();
        if text.is_empty() || text.len() > 16000 {
            return Err("invalid local model response length".into());
        }
        Ok(text.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_or_wrong_sized_model_folders_are_reported_missing() {
        let d = std::env::temp_dir().join(format!("reaction-models-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        assert_eq!(missing_models(&d), vec!["whisper-tiny", "kokoro-82m", "vision", "writer"]);
        let writer = d.join("writer");
        fs::create_dir_all(&writer).unwrap();
        fs::write(writer.join(WRITER_FILES[0].name), b"truncated").unwrap();
        assert!(missing_models(&d).contains(&"writer"));
        let f = fs::File::create(writer.join(WRITER_FILES[0].name)).unwrap();
        f.set_len(WRITER_FILES[0].size).unwrap();
        drop(f);
        assert!(!missing_models(&d).contains(&"writer"));
        let _ = fs::remove_dir_all(&d);
    }
}

use filmcraft_reaction_studio::{Analysis, Progress, Result, Script, analysis, providers, read_json, render, save_json};
use std::path::Path;

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let p = Progress { echo: true, ..Progress::default() };
    let get = |n: usize| -> Result<&str> {
        args.get(n).map(String::as_str).ok_or_else(||format!("missing argument {n}; commands: setup | analyze SOURCE OUT | script ANALYSIS OUT PERSPECTIVE INTENSITY COUNT | render ANALYSIS SCRIPT OUT --approve | fixture OUT").into())
    };
    match get(1)? {
        "setup" => providers::setup(&p)?,
        "analyze" => {
            let a = analysis::analyze(Path::new(get(2)?), Path::new(get(3)?), &p)?;
            println!("Analyzed {} evidence passages and {} sampled frames", a.evidence.len(), a.visuals.len());
        }
        "script" => {
            let a: Analysis = read_json(Path::new(get(2)?))?;
            let script = analysis::generate(&a, get(4)?, get(5)?.parse()?, "", get(6)?.parse()?, &p)?;
            save_json(Path::new(get(3)?), &script)?;
            println!("Draft saved; review before rendering");
        }
        "render" => {
            let a: Analysis = read_json(Path::new(get(2)?))?;
            let s: Script = read_json(Path::new(get(3)?))?;
            render::render(&a, &s, Path::new(get(4)?), "kokoro-heart", "Violet", get(5)? == "--approve", &p)?;
        }
        "fixture" => {
            let out = Path::new(get(2)?);
            std::fs::create_dir_all(out)?;
            let mut s = filmcraft_engine::Session::default();
            filmcraft_reaction_studio::exec(&mut s, "file.openDemoProject", serde_json::json!({}))?;
            let text = "The sun looks small in this animation. The video shows water reflecting light. Does that prove the water is safe to drink? No. Appearance alone cannot establish water quality.";
            let narration = filmcraft_reaction_studio::exec(
                &mut s,
                "tts.create",
                serde_json::json!({"text":text,"voice":"kokoro-heart","time":0,"dir":out.join("assets")}),
            )?;
            let duration = narration.get("duration").and_then(serde_json::Value::as_i64).ok_or("missing fixture narration duration")?;
            filmcraft_reaction_studio::exec(
                &mut s,
                "file.exportMedia",
                serde_json::json!({"path":out.join("spoken-source.mp4"),"format":"h264","width":640,"height":360,"fps":24,"range":"custom","startSeconds":0,"endSeconds":duration as f64/filmcraft_reaction_studio::TICKS as f64+0.5,"hardwareEncoding":"off","wait":true}),
            )?;
            std::fs::write(out.join("reference.txt"), text)?;
        }
        _ => return Err("unknown command".into()),
    }
    Ok(())
}
fn main() {
    match filmcraft_reaction_studio::bootstrap() {
        Ok(Some(code)) => std::process::exit(code),
        Ok(None) => {}
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1)
        }
    }
    if let Err(e) = run() {
        eprintln!("reaction-pipeline: {e}");
        std::process::exit(1);
    }
}

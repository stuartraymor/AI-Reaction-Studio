#![cfg_attr(windows, windows_subsystem = "windows")]
use egui::{Color32, RichText};
use filmcraft_reaction_studio::{Analysis, Progress, Result, Script, analysis, data_dir, providers, read_json, render, save_json};
use std::{
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    sync::{Arc, Mutex},
};

enum Done {
    Models,
    Analyzed(Analysis, PathBuf),
    Script(Script),
    Rendered(PathBuf),
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Settings {
    source: String,
    projects: String,
    perspective: String,
    intensity: u8,
    instructions: String,
    voice: String,
    character: String,
    count: usize,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            source: String::new(),
            projects: data_dir().parent().unwrap_or(Path::new(".")).join("Projects").to_string_lossy().into_owned(),
            perspective: "Neutral".into(),
            intensity: 30,
            instructions: String::new(),
            voice: "kokoro-heart".into(),
            character: "Violet".into(),
            count: 2,
        }
    }
}
struct Studio {
    settings: Settings,
    analysis: Option<Analysis>,
    script: Option<Script>,
    folder: Option<PathBuf>,
    rendered: Option<PathBuf>,
    approved: bool,
    progress: Progress,
    pending: Arc<Mutex<Option<std::result::Result<Done, String>>>>,
    running: bool,
    status: String,
    texture: Option<egui::TextureHandle>,
    screenshot: Option<PathBuf>,
    shot_requested: bool,
    frames: u32,
    closing: bool,
}
impl Studio {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut style = (*cc.egui_ctx.style_of(egui::Theme::Dark)).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 9.0);
        style.visuals.selection.bg_fill = Color32::from_rgb(99, 75, 205);
        cc.egui_ctx.set_style_of(egui::Theme::Dark, style);
        let settings = read_json(&data_dir().join("settings.json")).unwrap_or_default();
        let args: Vec<String> = std::env::args().collect();
        let screenshot = args.iter().position(|x| x == "--screenshot").and_then(|i| args.get(i + 1)).map(PathBuf::from);
        let mut app = Self {
            settings,
            analysis: None,
            script: None,
            folder: None,
            rendered: None,
            approved: false,
            progress: Progress::default(),
            pending: Arc::new(Mutex::new(None)),
            running: false,
            status: "Choose a local video, then analyze it.".into(),
            texture: None,
            screenshot,
            shot_requested: false,
            frames: 0,
            closing: false,
        };
        if let Some(path) = args.iter().position(|x| x == "--analysis").and_then(|i| args.get(i + 1)) {
            app.load_analysis(Path::new(path));
        }
        app
    }
    fn load_analysis(&mut self, path: &Path) {
        match read_json::<Analysis>(path) {
            Ok(a) => {
                self.settings.source = a.source.to_string_lossy().trim_start_matches("\\\\?\\").into();
                self.analysis = Some(a);
                self.folder = path.parent().map(Path::to_path_buf);
                self.script = self.folder.as_ref().and_then(|p| read_json(&p.join("script.json")).ok());
                if let Some(s) = &self.script {
                    self.settings.perspective = s.perspective.clone();
                    self.settings.intensity = s.intensity;
                    self.settings.instructions = s.custom_instructions.clone();
                }
                self.approved = false;
                self.status = "Saved analysis loaded. Review its limitations before generating a script.".into();
                self.texture = None;
            }
            Err(e) => self.status = e.to_string(),
        }
    }
    fn job(&mut self, f: impl FnOnce(Progress) -> Result<Done> + Send + 'static) {
        if self.running {
            return;
        }
        self.running = true;
        self.progress = Progress::default();
        self.status = "Processing locally…".into();
        let p = self.progress.clone();
        let pending = self.pending.clone();
        if let Err(e) = std::thread::Builder::new().name("reaction-production".into()).spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(p)))
                .unwrap_or_else(|_| Err("Production worker stopped unexpectedly; saved files remain recoverable.".into()))
                .map_err(|e| e.to_string());
            *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
        }) {
            self.running = false;
            self.status = e.to_string();
        }
    }
    fn poll(&mut self) {
        let result = self.pending.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(result) = result {
            self.running = false;
            match result {
                Ok(Done::Models) => self.status = "Local models ready. Choose Analyze video.".into(),
                Ok(Done::Analyzed(a, path)) => {
                    self.analysis = Some(a);
                    self.folder = Some(path);
                    self.script = None;
                    self.texture = None;
                    self.status = "Analysis ready. Inspect the transcript and sampled frames, then generate commentary.".into();
                }
                Ok(Done::Script(s)) => {
                    self.script = Some(s);
                    self.approved = false;
                    self.status = "Draft ready. Edit the commentary and approve it before rendering.".into();
                }
                Ok(Done::Rendered(path)) => {
                    self.rendered = Some(path);
                    self.status = "Finished MP4 and editable project saved.".into();
                }
                Err(e) => self.status = format!("Stopped: {e} — saved analysis/assets remain available for retry."),
            }
        }
    }
    fn content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.heading("AI Reaction Studio");
            ui.label(RichText::new("0.2 • DEVELOPMENT PREVIEW").color(Color32::from_rgb(176, 155, 255)));
        });
        ui.label("Local video analysis > evidence-linked commentary > reviewed script > editable reaction video");
        ui.separator();
        ui.horizontal(|ui| {
            ui.strong("Source video");
            if ui.add_enabled(!self.running, egui::Button::new("Browse…")).clicked()
                && let Some(p) = rfd::FileDialog::new().add_filter("Video", &["mp4", "mov", "mkv", "webm", "avi"]).pick_file()
            {
                self.settings.source = p.to_string_lossy().into_owned();
                self.analysis = None;
                self.script = None;
                self.texture = None;
                self.rendered = None;
            }
            if ui.add_enabled(!self.running, egui::Button::new("Open saved analysis…")).clicked()
                && let Some(p) = rfd::FileDialog::new().add_filter("Analysis report", &["json"]).pick_file()
            {
                self.load_analysis(&p);
            }
        });
        if ui
            .add_enabled(
                !self.running,
                egui::TextEdit::singleline(&mut self.settings.source).desired_width(f32::INFINITY).hint_text("Local video file • 3 seconds to 15 minutes"),
            )
            .changed()
        {
            self.analysis = None;
            self.script = None;
            self.texture = None;
        }
        ui.horizontal(|ui| {
            ui.label("Project folder");
            ui.add(egui::TextEdit::singleline(&mut self.settings.projects).desired_width(f32::INFINITY));
        });
        ui.horizontal(|ui| {
            ui.label("Perspective");
            egui::ComboBox::from_id_salt("perspective").selected_text(&self.settings.perspective).show_ui(ui, |ui| {
                for mode in ["Supportive", "Opposing", "Neutral", "Alternative", "Debate", "Comedy", "Educational", "Custom"] {
                    ui.selectable_value(&mut self.settings.perspective, mode.into(), mode);
                }
            });
            ui.label("Voice");
            egui::ComboBox::from_id_salt("voice").selected_text(&self.settings.voice).show_ui(ui, |ui| {
                for (id, name) in [("kokoro-heart", "Heart • female"), ("kokoro-michael", "Michael • male"), ("kokoro-puck", "Puck • male")] {
                    ui.selectable_value(&mut self.settings.voice, id.into(), name);
                }
            });
            egui::ComboBox::from_id_salt("character").selected_text(&self.settings.character).show_ui(ui, |ui| {
                for c in ["Violet", "Teal"] {
                    ui.selectable_value(&mut self.settings.character, c.into(), c);
                }
            });
        });
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut self.settings.intensity, 0..=100).text("Confrontation intensity"));
            ui.add(egui::Slider::new(&mut self.settings.count, 1..=4).text("Reactions"));
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.settings.instructions)
                .desired_rows(2)
                .char_limit(2000)
                .desired_width(f32::INFINITY)
                .hint_text("Optional personality or perspective instructions"),
        );
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(!self.running, egui::Button::new("Download local models (~3.6 GB)")).clicked() {
                self.job(|p| {
                    providers::setup(&p)?;
                    Ok(Done::Models)
                });
            }
            if ui.add_enabled(!self.running && !self.settings.source.trim().is_empty(), egui::Button::new("1  Analyze video")).clicked() {
                let source = PathBuf::from(&self.settings.source);
                let out = PathBuf::from(&self.settings.projects).join(format!("project-{}", stamp()));
                self.job(move |p| {
                    let a = analysis::analyze(&source, &out, &p)?;
                    Ok(Done::Analyzed(a, out))
                });
            }
            if ui.add_enabled(!self.running && self.analysis.is_some(), egui::Button::new("2  Generate script")).clicked()
                && let (Some(a), Some(folder)) = (self.analysis.clone(), self.folder.clone())
            {
                let (mode, intensity, instructions, count) =
                    (self.settings.perspective.clone(), self.settings.intensity, self.settings.instructions.clone(), self.settings.count);
                self.job(move |p| {
                    let script = analysis::generate(&a, &mode, intensity, &instructions, count, &p)?;
                    save_json(&folder.join("script.json"), &script)?;
                    Ok(Done::Script(script))
                });
            }
            if self.running && ui.button("Cancel").clicked() {
                self.progress.cancel.store(true, Ordering::Relaxed);
                self.status = "Cancellation requested; the current transcription/model call may need to finish.".into();
            }
        });
        ui.small("Models: Whisper tiny, SmolVLM2, Qwen3-4B and Kokoro • CPU mode • No cloud uploads or API charges");
        ui.small("Output: 640×360 at 24 fps • Pause and React • English commentary • Model licenses: MIT / Apache-2.0; CMUdict BSD");
        if let Some(a) = &self.analysis {
            ui.separator();
            ui.strong(format!(
                "Source analysis • {:.1}s • {} cuts • {} evidence passages • {} sampled frames",
                a.duration as f64 / filmcraft_reaction_studio::TICKS as f64,
                a.scene_cuts.len(),
                a.evidence.len(),
                a.visuals.len()
            ));
            if self.texture.is_none()
                && let Some(path) = self.folder.as_ref().and_then(|p| a.visuals.first().map(|v| p.join(&v.image)))
                && let Ok(img) = image::open(path)
            {
                let img = img.thumbnail(480, 270).to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                self.texture = Some(ctx.load_texture("source-frame", egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), Default::default()));
            }
            ui.horizontal(|ui|{if let Some(texture)=&self.texture{ui.image((texture.id(),egui::vec2(288.0,162.0)));}ui.vertical(|ui|{ui.label(RichText::new("Machine observations need review").color(Color32::from_rgb(240,192,100)));ui.label("Transcription may contain errors. Speaker identities are not inferred. Frames are sampled, not an exhaustive viewing. Generated opinions have not been independently fact checked.");});});
            egui::CollapsingHeader::new("Timestamped evidence and frame descriptions").show(ui, |ui| {
                for e in &a.evidence {
                    ui.label(format!(
                        "E{} • {:.2}–{:.2}s • {}\n{}",
                        e.id,
                        e.start as f64 / filmcraft_reaction_studio::TICKS as f64,
                        e.end as f64 / filmcraft_reaction_studio::TICKS as f64,
                        e.kind,
                        e.text
                    ));
                }
                for v in &a.visuals {
                    ui.label(format!(
                        "Frame {} • {:.2}s • {}\n{}",
                        v.id,
                        v.timestamp as f64 / filmcraft_reaction_studio::TICKS as f64,
                        v.status,
                        v.description
                    ));
                }
                for w in &a.warnings {
                    ui.colored_label(Color32::from_rgb(240, 192, 100), w);
                }
            });
        }
        if let Some(script) = &mut self.script {
            ui.separator();
            ui.heading("Review commentary");
            ui.label(&script.summary);
            egui::CollapsingHeader::new("Tentative argument analysis • unverified").show(ui, |ui| {
                ui.label(format!("Tentative tone: {}", script.argument_analysis.tentative_tone));
                for claim in &script.argument_analysis.claims {
                    ui.label(format!("E{} • {} • {}", claim.evidence_id, claim.kind, claim.interpretation));
                }
                for technique in &script.argument_analysis.tentative_rhetorical_techniques {
                    ui.label(format!("Possible rhetorical technique: {technique}"));
                }
            });
            let mut edited = false;
            for r in &mut script.segments {
                ui.strong(format!("At source {:.2}s • {} • unverified opinion", r.at as f64 / filmcraft_reaction_studio::TICKS as f64, script.perspective));
                egui::CollapsingHeader::new(format!("Source evidence E{}", r.evidence_id)).id_salt(r.evidence_id).show(ui, |ui| {
                    ui.label(&r.source_quote);
                });
                if ui.add(egui::TextEdit::multiline(&mut r.commentary).desired_rows(2).char_limit(1500).desired_width(f32::INFINITY)).changed() {
                    self.approved = false;
                    edited = true;
                }
            }
            if edited
                && let Some(folder) = &self.folder
                && let Err(e) = save_json(&folder.join("script.json"), script)
            {
                self.status = format!("Could not save script edits: {e}");
            }
            ui.checkbox(&mut self.approved, "I reviewed this script and approve local rendering");
        }
        let settings_match = self.script.as_ref().is_some_and(|s| {
            s.perspective == self.settings.perspective && s.intensity == self.settings.intensity && s.custom_instructions == self.settings.instructions
        });
        if self.script.is_some() && !settings_match {
            ui.label("Generate a new draft to apply the changed perspective, intensity or instructions.");
        }
        if ui.add_enabled(!self.running && self.approved && settings_match, egui::Button::new(RichText::new("3  Generate reaction video").strong())).clicked()
            && let (Some(a), Some(script), Some(folder)) = (self.analysis.clone(), self.script.clone(), self.folder.clone())
        {
            let out = folder.join(format!("render-{}", stamp()));
            let (voice, character) = (self.settings.voice.clone(), self.settings.character.clone());
            self.job(move |p| {
                render::render(&a, &script, &out, &voice, &character, true, &p)?;
                Ok(Done::Rendered(out))
            });
        }
        if let Some(out) = &self.rendered {
            ui.horizontal(|ui| {
                if ui.button("Preview MP4").clicked()
                    && let Err(e) = open::that(out.join("reaction.mp4"))
                {
                    self.status = e.to_string();
                }
                if ui.button("Open project folder").clicked()
                    && let Err(e) = open::that(out)
                {
                    self.status = e.to_string();
                }
                if ui.button("Edit timeline in FilmCraft").clicked() {
                    let result = (|| -> Result<()> {
                        let editor = std::env::current_exe()?.parent().ok_or("application folder missing")?.join("editor").join("filmcraft.exe");
                        std::process::Command::new(editor).arg("--data-dir").arg(data_dir().join("Editor")).arg(out.join("reaction.fcproj")).spawn()?;
                        Ok(())
                    })();
                    if let Err(e) = result {
                        self.status = e.to_string();
                    }
                }
            });
            ui.small(out.display().to_string());
        }
        ui.separator();
        ui.label(RichText::new(&self.status).strong());
        if self.running {
            ui.spinner();
        }
        let messages = self.progress.messages.lock().unwrap_or_else(|e| e.into_inner());
        egui::ScrollArea::vertical().id_salt("production-log").max_height(100.0).stick_to_bottom(true).show(ui, |ui| {
            for line in messages.iter().rev().take(12).rev() {
                ui.small(line);
            }
        });
    }
}
fn stamp() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}
impl eframe::App for Studio {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.poll();
        self.frames = self.frames.saturating_add(1);
        egui::CentralPanel::default().show(root, |ui| {
            egui::ScrollArea::vertical().id_salt("workspace").show(ui, |ui| self.content(ui, &ctx));
        });
        if ctx.input(|i| i.viewport().close_requested()) && self.running {
            self.closing = true;
            self.progress.cancel.store(true, Ordering::Relaxed);
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if self.closing && !self.running {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.screenshot.is_some() && self.frames > 40 && !self.shot_requested {
            self.shot_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            if let egui::Event::Screenshot { image, .. } = event
                && let Some(path) = &self.screenshot
            {
                let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                if let Some(img) = image::RgbaImage::from_raw(image.width() as u32, image.height() as u32, rgba)
                    && let Err(e) = img.save(path)
                {
                    self.status = e.to_string();
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(if self.running { 100 } else { 250 }));
    }
    fn on_exit(&mut self) {
        let _ = save_json(&data_dir().join("settings.json"), &self.settings);
    }
}
fn start() -> std::result::Result<(), Box<dyn std::error::Error>> {
    match filmcraft_reaction_studio::bootstrap() {
        Ok(Some(code)) => std::process::exit(code),
        Ok(None) => {}
        Err(e) => {
            return Err(e.to_string().into());
        }
    }
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1060.0, 740.0]).with_min_inner_size([760.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native("AI Reaction Studio", native, Box::new(|cc| Ok(Box::new(Studio::new(cc)))))?;
    Ok(())
}
fn main() {
    if let Err(e) = start() {
        let message = format!("AI Reaction Studio could not start: {e}");
        let log = data_dir().join("Logs");
        let _ = std::fs::create_dir_all(&log);
        let _ = std::fs::write(log.join("startup-error.txt"), &message);
        rfd::MessageDialog::new().set_title("AI Reaction Studio startup error").set_description(message).set_level(rfd::MessageLevel::Error).show();
    }
}

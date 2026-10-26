#![cfg_attr(windows, windows_subsystem = "windows")]
use egui::{Color32, RichText};
use filmcraft_reaction_studio::source::{SourceInput, parse_source};
use filmcraft_reaction_studio::style::{self, Mood};
use filmcraft_reaction_studio::{Analysis, Progress, Result, Script, avatar, data_dir, download, pipeline, providers, read_json, render, revision, save_json};
use std::{
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    sync::{Arc, Mutex},
};

enum Done {
    Models,
    Drafted(Analysis, Script, PathBuf),
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
    subtitles: bool,
    /// Reaction style id (`style.rs`) or "custom"; empty in settings saved before 0.4.
    style: String,
    /// The optional "Anything else?" line added to the chosen style.
    extra: String,
}
/// Settings from disk, or first-run defaults. Pre-0.4 files get the preset closest to what they used.
fn load_settings(saved: Result<Settings>) -> Settings {
    match saved {
        Ok(mut s) => {
            if s.style.is_empty() {
                s.style = style::migrate(&s.perspective, s.intensity).into();
            }
            if s.style != style::CUSTOM && style::find(&s.style).is_none() {
                s.style = style::DEFAULT_STYLE.into();
            }
            s.count = s.count.clamp(1, 4);
            s
        }
        Err(_) => {
            let mut s = Settings { style: style::DEFAULT_STYLE.into(), count: 3, ..Settings::default() };
            s.apply_style(style::DEFAULT_STYLE);
            s
        }
    }
}
impl Settings {
    /// Selecting a card copies its values into the Advanced controls, so they show what it does.
    fn apply_style(&mut self, id: &str) {
        if let Some(st) = style::find(id) {
            self.style = st.id.into();
            self.perspective = st.perspective.into();
            self.intensity = st.intensity;
            self.instructions = st.instructions.into();
        }
    }
    fn writer(&self) -> pipeline::Writer {
        style::resolve(&self.style, &self.perspective, self.intensity, &self.instructions, &self.extra)
    }
    fn matches(&self, script: &Script) -> bool {
        let (perspective, intensity, instructions) = self.writer();
        script.perspective == perspective && script.intensity == intensity && script.custom_instructions == instructions && script.style == self.style
    }
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
            subtitles: true,
            style: String::new(),
            extra: String::new(),
        }
    }
}
const VIDEO_EXTENSIONS: [&str; 6] = ["mp4", "mov", "m4v", "mkv", "webm", "avi"];
#[derive(Debug, PartialEq)]
enum Dropped {
    Video(PathBuf),
    Analysis(PathBuf),
    Unsupported(String),
}
/// Decides what a file dropped onto the window means: a source video or a saved analysis report.
fn classify_drop(path: &Path) -> Dropped {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        Dropped::Video(path.to_path_buf())
    } else if ext == "json" {
        Dropped::Analysis(path.to_path_buf())
    } else {
        Dropped::Unsupported(path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
    }
}
/// The line under the video box: whether it is usable and what it was recognised as.
fn source_hint(text: &str) -> Option<(bool, String)> {
    if text.trim().is_empty() {
        return None;
    }
    Some(match parse_source(text) {
        Ok(SourceInput::YouTube { id }) => (true, format!("YouTube video {id} • downloaded with your yt-dlp. {}", download::RIGHTS_NOTICE)),
        Ok(SourceInput::File(path)) if path.is_file() => (true, "Video file on this computer".into()),
        Ok(SourceInput::File(_)) => (false, "That file doesn't exist. Check the path or use Browse…".into()),
        Err(e) => (false, e.to_string()),
    })
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
    revision_directions: String,
    screenshot_review: bool,
    /// First-run checks: model groups still missing, and whether yt-dlp was found.
    missing_models: Vec<&'static str>,
    has_downloader: bool,
}
impl Studio {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut style = (*cc.egui_ctx.style_of(egui::Theme::Dark)).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 9.0);
        style.visuals.selection.bg_fill = Color32::from_rgb(99, 75, 205);
        cc.egui_ctx.set_style_of(egui::Theme::Dark, style);
        let settings = load_settings(read_json(&data_dir().join("settings.json")));
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
            status: "Paste a YouTube link or drop a video file, pick how it reacts and choose Make my reaction.".into(),
            texture: None,
            screenshot,
            shot_requested: false,
            frames: 0,
            closing: false,
            revision_directions: String::new(),
            screenshot_review: args.iter().any(|a| a == "--screenshot-review"),
            missing_models: Vec::new(),
            has_downloader: false,
        };
        app.check_setup();
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
                    let id = if s.style.is_empty() { style::migrate(&s.perspective, s.intensity) } else { s.style.as_str() };
                    match style::find(id) {
                        Some(st) => {
                            self.settings.apply_style(st.id);
                            self.settings.extra = s.custom_instructions.strip_prefix(st.instructions).unwrap_or("").trim().into();
                        }
                        None => {
                            self.settings.style = style::CUSTOM.into();
                            self.settings.perspective = s.perspective.clone();
                            self.settings.intensity = s.intensity;
                            self.settings.instructions = s.custom_instructions.clone();
                            self.settings.extra.clear();
                        }
                    }
                }
                self.approved = false;
                self.status = "Saved analysis loaded. Review its limitations before generating a script.".into();
                self.texture = None;
            }
            Err(e) => self.status = e.to_string(),
        }
    }
    fn check_setup(&mut self) {
        self.missing_models = providers::missing_models(&data_dir().join("models"));
        self.has_downloader = download::downloader_path().is_ok();
    }
    fn set_source(&mut self, text: String) {
        self.settings.source = text;
        self.analysis = None;
        self.script = None;
        self.texture = None;
        self.rendered = None;
    }
    fn dropped(&mut self, path: &Path) {
        match classify_drop(path) {
            Dropped::Video(p) => self.set_source(p.to_string_lossy().into_owned()),
            Dropped::Analysis(p) => self.load_analysis(&p),
            Dropped::Unsupported(name) => self.status = format!("{name} isn't a video. Drop an MP4, MOV, MKV, WebM or AVI file, or a saved analysis.json."),
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
                Ok(Done::Models) => {
                    self.check_setup();
                    self.status = "Local models ready. Paste a link or drop a video, pick a style and choose Make my reaction.".into();
                }
                Ok(Done::Drafted(a, script, folder)) => {
                    let same = self.folder.as_ref() == Some(&folder);
                    self.analysis = Some(a);
                    self.folder = Some(folder);
                    self.script = Some(script);
                    self.approved = false;
                    self.rendered = None;
                    if !same {
                        self.texture = None;
                    }
                    self.status = "Here's what it will say. Edit anything you like, tick OK, then make the video.".into();
                }
                Ok(Done::Script(s)) => {
                    self.script = Some(s);
                    self.approved = false;
                    self.rendered = None;
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
    /// The one-button path: link/file → (download) → analysis (reused when it matches) → draft.
    fn make(&mut self) {
        let folder = match (&self.analysis, &self.folder) {
            (Some(_), Some(f)) => f.clone(),
            _ => PathBuf::from(&self.settings.projects).join(format!("project-{}", pipeline::stamp())),
        };
        let (source, id, writer, count) = (self.settings.source.clone(), self.settings.style.clone(), self.settings.writer(), self.settings.count);
        self.job(move |p| {
            let (a, script, _) = pipeline::draft(&source, &folder, &id, &writer, count, &p)?;
            Ok(Done::Drafted(a, script, folder))
        });
    }
    fn style_cards(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = ctx.input(|i| i.time);
        let color = if self.settings.character == "Teal" { Color32::from_rgb(8, 127, 130) } else { Color32::from_rgb(112, 77, 232) };
        // Each group frame adds ~14 px of margin and stroke, plus 10 px item spacing between cards.
        let width = ((ui.available_width() - 6.0 * 26.0) / 6.0).clamp(80.0, 170.0);
        ui.horizontal_wrapped(|ui| {
            for st in style::STYLES {
                let selected = self.settings.style == st.id;
                let frame = egui::Frame::group(ui.style()).fill(if selected { Color32::from_rgb(58, 44, 120) } else { ui.visuals().extreme_bg_color }).stroke(
                    egui::Stroke::new(if selected { 2.0 } else { 1.0 }, if selected { Color32::from_rgb(176, 155, 255) } else { Color32::from_gray(70) }),
                );
                let response = ui
                    .push_id(("style-card", st.id), |ui| {
                        frame
                            .show(ui, |ui| {
                                ui.set_width(width);
                                ui.vertical_centered(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
                                    paint_figure(ui.painter(), rect, color, st.mood, style::motion_gain(&st), t);
                                    ui.strong(st.label);
                                    ui.small(st.blurb);
                                });
                            })
                            .response
                            .interact(egui::Sense::click())
                    })
                    .inner;
                if response.clicked() && !self.running {
                    self.settings.apply_style(st.id);
                }
            }
        });
        if self.settings.style == style::CUSTOM {
            ui.small(RichText::new("Custom: using your Advanced settings").color(Color32::from_rgb(176, 155, 255)));
        }
    }
    fn content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.heading("AI Reaction Studio");
            ui.label(RichText::new("0.4 • DEVELOPMENT PREVIEW").color(Color32::from_rgb(176, 155, 255)));
        });
        ui.label("1  Paste a link or drop a video    2  Pick how it reacts    3  Make my reaction");
        if !self.missing_models.is_empty() || !self.has_downloader {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.strong("Getting started");
                ui.horizontal_wrapped(|ui| {
                    if self.missing_models.is_empty() {
                        ui.label(RichText::new("✔ Local AI models ready").color(Color32::from_rgb(140, 200, 150)));
                    } else {
                        ui.label(
                            RichText::new(format!("✖ Local AI models missing: {}", self.missing_models.join(", "))).color(Color32::from_rgb(240, 192, 100)),
                        );
                        if ui.add_enabled(!self.running, egui::Button::new("Download local models (~3.6 GB)")).clicked() {
                            self.job(|p| {
                                providers::setup(&p)?;
                                Ok(Done::Models)
                            });
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if self.has_downloader {
                        ui.label(RichText::new("✔ yt-dlp found: YouTube links work").color(Color32::from_rgb(140, 200, 150)));
                    } else {
                        ui.label("○ Optional: install yt-dlp to use YouTube links (files work without it)");
                        if ui.button("How to install yt-dlp").clicked()
                            && let Err(e) = open::that("https://github.com/yt-dlp/yt-dlp#installation")
                        {
                            self.status = e.to_string();
                        }
                    }
                    if ui.add_enabled(!self.running, egui::Button::new("Check again")).clicked() {
                        self.check_setup();
                    }
                });
            });
        }
        ui.separator();
        ui.strong("Video");
        ui.horizontal(|ui| {
            let edit = ui.add_enabled(
                !self.running,
                egui::TextEdit::singleline(&mut self.settings.source)
                    .id_salt("source-input")
                    .desired_width((ui.available_width() - 90.0).max(120.0))
                    .hint_text("Paste a YouTube link or drop a video file • up to 15 minutes"),
            );
            if edit.changed() {
                let text = std::mem::take(&mut self.settings.source);
                self.set_source(text);
            }
            if ui.add_enabled(!self.running, egui::Button::new("Browse…")).clicked()
                && let Some(p) = rfd::FileDialog::new().add_filter("Video", &VIDEO_EXTENSIONS).pick_file()
            {
                self.set_source(p.to_string_lossy().into_owned());
            }
        });
        let source_ok = match source_hint(&self.settings.source) {
            Some((ok, hint)) => {
                ui.small(RichText::new(hint).color(if ok { Color32::from_rgb(140, 200, 150) } else { Color32::from_rgb(240, 192, 100) }));
                ok
            }
            None => false,
        };
        ui.add_space(4.0);
        ui.strong("How should it react?");
        self.style_cards(ui, ctx);
        ui.horizontal(|ui| {
            ui.label("Anything else?");
            ui.add_enabled(
                !self.running,
                egui::TextEdit::singleline(&mut self.settings.extra)
                    .id_salt("style-extra")
                    .char_limit(style::MAX_EXTRA)
                    .desired_width(f32::INFINITY)
                    .hint_text("Optional, e.g. \"keep it short\" or \"talk like a sports commentator\""),
            );
        });
        egui::CollapsingHeader::new("Advanced").id_salt("advanced").show(ui, |ui| {
            let before = (self.settings.perspective.clone(), self.settings.intensity, self.settings.instructions.clone());
            ui.horizontal(|ui| {
                ui.label("Project folder");
                ui.add(egui::TextEdit::singleline(&mut self.settings.projects).id_salt("project-folder").desired_width(f32::INFINITY));
            });
            ui.horizontal(|ui| {
                ui.label("Perspective");
                egui::ComboBox::from_id_salt("perspective").selected_text(&self.settings.perspective).show_ui(ui, |ui| {
                    for mode in filmcraft_reaction_studio::analysis::PERSPECTIVES {
                        ui.selectable_value(&mut self.settings.perspective, mode.into(), mode);
                    }
                });
                ui.label("Voice");
                egui::ComboBox::from_id_salt("voice").selected_text(&self.settings.voice).show_ui(ui, |ui| {
                    for (id, name) in [("kokoro-heart", "Heart • female"), ("kokoro-michael", "Michael • male"), ("kokoro-puck", "Puck • male")] {
                        ui.selectable_value(&mut self.settings.voice, id.into(), name);
                    }
                });
                ui.label("Figure");
                egui::ComboBox::from_id_salt("character").selected_text(&self.settings.character).show_ui(ui, |ui| {
                    for c in ["Violet", "Teal"] {
                        ui.selectable_value(&mut self.settings.character, c.into(), c);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut self.settings.intensity, 0..=100).text("Intensity"));
                ui.add(egui::Slider::new(&mut self.settings.count, 1..=4).text("Reactions"));
            });
            ui.add(
                egui::TextEdit::multiline(&mut self.settings.instructions)
                    .id_salt("advanced-instructions")
                    .desired_rows(2)
                    .char_limit(2000)
                    .desired_width(f32::INFINITY)
                    .hint_text("Personality or perspective instructions"),
            );
            ui.push_id("commentary-subtitles", |ui| ui.checkbox(&mut self.settings.subtitles, "Commentary subtitles (editable track + SRT; estimated timing)"));
            if (self.settings.perspective.clone(), self.settings.intensity, self.settings.instructions.clone()) != before {
                self.settings.style = style::CUSTOM.into();
            }
            if ui.add_enabled(!self.running, egui::Button::new("Open saved analysis…")).clicked()
                && let Some(p) = rfd::FileDialog::new().add_filter("Analysis report", &["json"]).pick_file()
            {
                self.load_analysis(&p);
            }
        });
        ui.horizontal_wrapped(|ui| {
            let label = if self.script.is_some() { "Make it again with this style" } else { "Make my reaction" };
            if ui
                .add_enabled(!self.running && source_ok, egui::Button::new(RichText::new(label).strong().size(17.0)).min_size(egui::vec2(220.0, 34.0)))
                .clicked()
            {
                self.make();
            }
            if self.running && ui.button("Cancel").clicked() {
                self.progress.cancel.store(true, Ordering::Relaxed);
                self.status = "Cancellation requested; downloads stop at once, a transcription/model call may need to finish.".into();
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
        let mut regenerate_index = None;
        if let Some(script) = &mut self.script {
            ui.separator();
            ui.heading("Here's what it will say");
            ui.label(&script.summary);
            ui.label("Revise one reaction while retaining its source quote and timestamp. Review each replacement before rendering.");
            ui.add_enabled(
                !self.running,
                egui::TextEdit::singleline(&mut self.revision_directions)
                    .id_salt("reaction-revision-directions")
                    .hint_text("Optional directions for the selected reaction")
                    .char_limit(1000)
                    .desired_width(f32::INFINITY),
            );
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
            let style_label = style::find(&script.style).map(|s| s.label).unwrap_or("Custom");
            let matching = self.settings.matches(script);
            for (index, r) in script.segments.iter_mut().enumerate() {
                ui.strong(format!("At {:.1}s • {style_label} • unverified opinion", r.at as f64 / filmcraft_reaction_studio::TICKS as f64));
                egui::CollapsingHeader::new(format!("Source evidence E{}", r.evidence_id)).id_salt(r.evidence_id).show(ui, |ui| {
                    ui.label(&r.source_quote);
                });
                if ui
                    .add_enabled(!self.running, egui::TextEdit::multiline(&mut r.commentary).desired_rows(2).char_limit(1500).desired_width(f32::INFINITY))
                    .changed()
                {
                    self.approved = false;
                    edited = true;
                    self.rendered = None;
                }
                if ui
                    .push_id(("regenerate-reaction", index), |ui| {
                        ui.add_enabled(!self.running && matching, egui::Button::new(format!("Regenerate reaction {}", index + 1))).clicked()
                    })
                    .inner
                {
                    regenerate_index = Some(index);
                }
            }
            if edited
                && let Some(folder) = &self.folder
                && let Err(e) = save_json(&folder.join("script.json"), script)
            {
                self.status = format!("Could not save script edits: {e}");
            }
            ui.add_enabled(!self.running, egui::Checkbox::new(&mut self.approved, "OK, I checked what it will say"));
        }
        if let Some(index) = regenerate_index
            && let (Some(a), Some(script), Some(folder)) = (self.analysis.clone(), self.script.clone(), self.folder.clone())
        {
            let directions = self.revision_directions.clone();
            self.approved = false;
            self.job(move |p| {
                let revised = revision::regenerate(&a, &script, index, &directions, &p)?;
                p.step("Saving replacement draft with recoverable previous-script backup")?;
                save_json(&folder.join("script.json"), &revised)?;
                Ok(Done::Script(revised))
            });
        }
        let settings_match = self.script.as_ref().is_some_and(|s| self.settings.matches(s));
        if self.script.is_some() && !settings_match {
            ui.label("You changed the style. Choose Make it again with this style to get a new draft (the analysis is reused).");
        }
        if self.script.is_some()
            && ui
                .add_enabled(!self.running && self.approved && settings_match, egui::Button::new(RichText::new("Make the video").strong().size(17.0)))
                .clicked()
            && let (Some(a), Some(script), Some(folder)) = (self.analysis.clone(), self.script.clone(), self.folder.clone())
        {
            let out = folder.join(format!("render-{}-{}", script.style, pipeline::stamp()));
            let (voice, character) = (self.settings.voice.clone(), self.settings.character.clone());
            let subtitles = self.settings.subtitles;
            self.job(move |p| {
                render::render_with_subtitles(&a, &script, &out, &voice, &character, true, subtitles, &p)?;
                Ok(Done::Rendered(out))
            });
        }
        let mut try_another = false;
        if let Some(out) = &self.rendered {
            ui.separator();
            ui.heading("Your reaction video is ready");
            ui.horizontal_wrapped(|ui| {
                if ui.button(RichText::new("▶ Watch").strong()).clicked()
                    && let Err(e) = open::that(out.join("reaction.mp4"))
                {
                    self.status = e.to_string();
                }
                if ui.button("Save MP4…").clicked()
                    && let Some(dest) = rfd::FileDialog::new().set_file_name("reaction.mp4").add_filter("MP4 video", &["mp4"]).save_file()
                {
                    self.status = match std::fs::copy(out.join("reaction.mp4"), &dest) {
                        Ok(_) => format!("Saved {}", dest.display()),
                        Err(e) => format!("Could not save the video: {e}"),
                    };
                }
                if ui.button("Try another style").clicked() {
                    try_another = true;
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
        if try_another {
            self.script = None;
            self.rendered = None;
            self.approved = false;
            self.status = "Pick a different style, then choose Make my reaction. The video and analysis are reused.".into();
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
/// The style cards' live preview: the same poses the renderer keyframes, drawn with the painter.
fn paint_figure(painter: &egui::Painter, rect: egui::Rect, color: Color32, mood: Mood, gain: f32, t: f64) {
    let pose = avatar::pose(mood, gain, t.rem_euclid(6.0));
    let k = rect.width() / 130.0;
    let c = rect.center() + egui::vec2(0.0, pose.bob * k);
    painter.add(egui::Shape::ellipse_filled(c, egui::vec2(52.0 * k, 56.0 * k), color));
    let ink = Color32::from_rgb(23, 17, 38);
    for (i, side) in [-1.0f32, 1.0].iter().enumerate() {
        let eye = c + egui::vec2(21.0 * side * k, -17.0 * k);
        painter.add(egui::Shape::ellipse_filled(eye, egui::vec2(7.5 * k, (10.0 * pose.eye_open).max(1.0) * k), Color32::WHITE));
        let brow = eye + egui::vec2(0.0, (-18.0 - pose.brow_raise.get(i).copied().unwrap_or(0.0)) * k);
        let angle = pose.brow_tilt.get(i).copied().unwrap_or(0.0).to_radians();
        let half = egui::vec2(angle.cos(), angle.sin()) * 11.0 * k;
        painter.line_segment([brow - half, brow + half], egui::Stroke::new(4.0 * k, ink));
    }
    let talk = ((t * 7.0).sin().abs() * 0.12) as f64;
    let mouth = avatar::mouth_height(talk, gain) as f32;
    painter.rect_filled(egui::Rect::from_center_size(c + egui::vec2(0.0, 27.0 * k), egui::vec2(38.0 * k, mouth * k)), 3.0 * k, ink);
}
impl eframe::App for Studio {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.poll();
        self.frames = self.frames.saturating_add(1);
        let (hovering, dropped) =
            ctx.input(|i| (!i.raw.hovered_files.is_empty(), i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).find(|p| !p.as_os_str().is_empty())));
        if !self.running
            && let Some(path) = dropped
        {
            self.dropped(&path);
        }
        egui::CentralPanel::default().show(root, |ui| {
            let scroll = egui::ScrollArea::vertical().id_salt("workspace");
            let scroll = if self.screenshot_review { scroll.vertical_scroll_offset(600.0) } else { scroll };
            scroll.show(ui, |ui| self.content(ui, &ctx));
        });
        if hovering {
            let screen = ctx.content_rect();
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop-overlay")));
            painter.rect_filled(screen, 0.0, Color32::from_black_alpha(180));
            let text = if self.running { "Busy — wait for the current step to finish" } else { "Drop a video (or analysis.json) to use it" };
            painter.text(screen.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(26.0), Color32::WHITE);
        }
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
        // ~30 fps keeps the style previews moving; the app is otherwise idle.
        ctx.request_repaint_after(std::time::Duration::from_millis(if self.script.is_none() {
            33
        } else if self.running {
            100
        } else {
            250
        }));
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
        viewport: egui::ViewportBuilder::default().with_inner_size([1060.0, 740.0]).with_min_inner_size([760.0, 600.0]).with_drag_and_drop(true),
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
#[cfg(test)]
mod source_box_tests {
    use super::{Dropped, classify_drop, source_hint};
    use std::path::{Path, PathBuf};
    #[test]
    fn drops_route_videos_and_analyses() {
        assert_eq!(classify_drop(Path::new(r"C:\clips\Talk.MP4")), Dropped::Video(PathBuf::from(r"C:\clips\Talk.MP4")));
        assert_eq!(classify_drop(Path::new("a/b.webm")), Dropped::Video(PathBuf::from("a/b.webm")));
        assert_eq!(classify_drop(Path::new("proj/analysis.json")), Dropped::Analysis(PathBuf::from("proj/analysis.json")));
        assert_eq!(classify_drop(Path::new("notes.txt")), Dropped::Unsupported("notes.txt".into()));
        assert_eq!(classify_drop(Path::new("no-extension")), Dropped::Unsupported("no-extension".into()));
        assert_eq!(classify_drop(Path::new("")), Dropped::Unsupported(String::new()));
    }
    #[test]
    fn hint_recognises_links_files_and_mistakes() {
        assert_eq!(source_hint("   "), None);
        let (ok, text) = source_hint("https://youtu.be/dQw4w9WgXcQ").unwrap();
        assert!(ok && text.contains("dQw4w9WgXcQ") && text.contains("permission"));
        assert!(!source_hint("https://youtube.com.evil.example/watch?v=dQw4w9WgXcQ").unwrap().0);
        let (ok, text) = source_hint(r"C:\definitely\missing\clip.mp4").unwrap();
        assert!(!ok && text.contains("doesn't exist"));
        let file = std::env::temp_dir().join(format!("reaction-hint-{}.mp4", std::process::id()));
        std::fs::write(&file, b"x").unwrap();
        assert!(source_hint(&format!("\"{}\"", file.display())).unwrap().0);
        let _ = std::fs::remove_file(&file);
    }
}
#[cfg(test)]
mod style_settings_tests {
    use super::{Settings, load_settings};
    use filmcraft_reaction_studio::{Script, style};
    #[test]
    fn first_run_starts_on_the_default_style() {
        let s = load_settings(Err("no file".into()));
        assert_eq!(s.style, style::DEFAULT_STYLE);
        assert_eq!(s.perspective, style::find(style::DEFAULT_STYLE).unwrap().perspective);
    }
    #[test]
    fn older_settings_migrate_to_a_style() {
        let old: Settings = serde_json::from_str(r#"{"perspective":"Comedy","intensity":70}"#).unwrap();
        assert_eq!(load_settings(Ok(old)).style, "funny");
        let alt: Settings = serde_json::from_str(r#"{"perspective":"Alternative","intensity":70}"#).unwrap();
        assert_eq!(load_settings(Ok(alt)).style, style::CUSTOM);
        let junk: Settings = serde_json::from_str(r#"{"style":"made-up","count":99}"#).unwrap();
        let junk = load_settings(Ok(junk));
        assert_eq!((junk.style.as_str(), junk.count), (style::DEFAULT_STYLE, 4));
    }
    #[test]
    fn picking_a_card_fills_the_advanced_controls() {
        let mut s = Settings::default();
        s.apply_style("skeptic");
        assert_eq!((s.style.as_str(), s.perspective.as_str(), s.intensity), ("skeptic", "Opposing", 55));
        s.apply_style("nope");
        assert_eq!(s.style, "skeptic", "unknown ids change nothing");
    }
    #[test]
    fn a_script_matches_only_the_settings_that_made_it() {
        let mut s = Settings::default();
        s.apply_style("funny");
        s.extra = "short".into();
        let (perspective, intensity, custom_instructions) = s.writer();
        let mut script: Script = serde_json::from_value(serde_json::json!({
            "schema_version":1,"source_sha256":"x","perspective":perspective,"intensity":intensity,"custom_instructions":custom_instructions,
            "style":"funny","review_required":true,"summary":"","segments":[],
            "argument_analysis":{"tentative_tone":"","tentative_rhetorical_techniques":[],"claims":[]}
        }))
        .unwrap();
        assert!(s.matches(&script));
        s.extra = "longer".into();
        assert!(!s.matches(&script));
        s.extra = "short".into();
        script.style = "hype".into();
        assert!(!s.matches(&script));
    }
}
#[cfg(test)]
mod settings_tests {
    use super::Settings;
    #[test]
    fn previous_settings_keep_choices_and_receive_caption_default() {
        let settings: Settings = serde_json::from_str(r#"{"perspective":"Educational","voice":"kokoro-michael","count":3}"#).unwrap();
        assert_eq!(settings.perspective, "Educational");
        assert_eq!(settings.voice, "kokoro-michael");
        assert_eq!(settings.count, 3);
        assert!(settings.subtitles);
    }
}

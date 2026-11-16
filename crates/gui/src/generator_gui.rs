use eframe::egui;
use rayon::prelude::*;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, TryRecvError, channel},
    },
    time::{Duration, Instant},
};

use generator_core::{
    bilingual_generator::BilingualGenerator,
    bilingual_generator_errors::BilingualGeneratorError,
    config::{AppConfig, pairs_to_text, parse_pairs_text},
    log::LogSink,
};

type GenerationResult = Result<Vec<String>, BilingualGeneratorError>;

/// Shared buffer the worker thread appends diagnostics to. Drained into `messages` every frame,
/// so warnings show up while generation is still running and survive a later failure.
type LogBuffer = Arc<Mutex<Vec<String>>>;

pub struct GuiBilingualPakGenerator {
    /// Everything that lives in `kcd_bilingual_generator_rust.json`: game path + pairs to emit.
    config: AppConfig,
    /// Absolute path of that file, so the window can show where it is.
    config_path: PathBuf,
    /// The pair list as the user sees it: one `Primary + Secondary` per line. Kept separate from
    /// `config.bilingual_set` so a half-typed line does not become a config change.
    bilingual_set_text: String,
    /// Why the text block currently does not parse, shown right under it.
    pairs_error: Option<String>,
    messages: String,
    /// The generation job currently running in the background, if any.
    pending: Option<PendingJob>,
}

/// Handle on a background generation job. Dropping the receiver is harmless:
/// the worker only ignores the send error and exits.
struct PendingJob {
    rx: Receiver<GenerationResult>,
    log: LogBuffer,
    started_at: Instant,
}

/// Outcome of a finished background job, resolved once per frame.
enum JobOutcome {
    Finished { lines: Vec<String>, elapsed: Duration },
    Failed { message: String, elapsed: Duration },
}

impl Default for GuiBilingualPakGenerator {
    fn default() -> Self {
        // The config file is the only setup a release build needs: it is created on first launch
        // (with the default pairs) and read back on every launch after that.
        let loaded = AppConfig::load_or_create(&Self::config_dir());
        let mut app = Self {
            bilingual_set_text: pairs_to_text(&loaded.config.bilingual_set),
            config: loaded.config,
            config_path: loaded.path,
            pairs_error: None,
            messages: String::new(),
            pending: None,
        };
        for note in loaded.notes {
            app.push_message(&note);
        }
        app.push_message(&format!("{} bilingual pair(s) configured", app.config.bilingual_set.len()));
        app
    }
}

impl GuiBilingualPakGenerator {
    /// Directory the config file lives in: the process working directory.
    fn config_dir() -> PathBuf {
        std::env::current_dir().unwrap_or_default()
    }

    /// Builds the app and applies the custom fonts/style exactly once.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_style(&cc.egui_ctx);
        Self::default()
    }

    /// Persists the config after an in-window edit. Failures are reported, never fatal: the
    /// generation itself does not depend on the file being writable.
    fn persist_config(&mut self) {
        if let Err(error) = self.config.save_to(&self.config_path) {
            let message = format!("Could not save {}: {error}", self.config_path.display());
            self.push_message(&message);
        }
    }

    /// Applies an edited game path and writes it straight away, so a crash or a shortcut that
    /// starts elsewhere cannot lose the setting.
    fn set_game_path(&mut self, path: String) {
        if self.config.game_path == path {
            return;
        }
        self.config.game_path = path;
        self.persist_config();
    }

    /// Re-reads the JSON file, picking up pairs the user added by editing it in a text editor.
    fn reload_config(&mut self) {
        let loaded = AppConfig::load_or_create(&Self::config_dir());
        self.config = loaded.config;
        self.config_path = loaded.path;
        self.bilingual_set_text = pairs_to_text(&self.config.bilingual_set);
        self.pairs_error = None;
        for note in loaded.notes {
            self.push_message(&note);
        }
        self.push_message(&format!("{} bilingual pair(s) configured", self.config.bilingual_set.len()));
    }

    /// Turns an edited text block into config pairs and saves it.
    ///
    /// A line that does not parse leaves the config (and the file) untouched and only shows an
    /// inline error, so typing never wipes the list the user is halfway through.
    fn apply_pairs_text(&mut self) {
        match parse_pairs_text(&self.bilingual_set_text) {
            Ok(pairs) => {
                self.pairs_error = None;
                if self.config.bilingual_set == pairs {
                    return;
                }
                self.config.bilingual_set = pairs;
                self.persist_config();
            }
            Err(error) => self.pairs_error = Some(error.to_string()),
        }
    }

    /// Appends one line, keeping the buffer newline-terminated.
    fn push_message(&mut self, line: &str) {
        if !self.messages.is_empty() && !self.messages.ends_with('\n') {
            self.messages.push('\n');
        }
        self.messages.push_str(line);
        self.messages.push('\n');
    }

    /// Drains worker diagnostics and, once the job finishes, reports its outcome.
    /// Requests a repaint while the job runs so warnings and the elapsed clock keep moving.
    fn poll_job(&mut self, ctx: &egui::Context) {
        let mut drained: Vec<String> = Vec::new();
        let mut outcome: Option<JobOutcome> = None;

        let Some(job) = self.pending.as_ref() else {
            return;
        };

        if let Ok(mut buffer) = job.log.lock() {
            drained = std::mem::take(&mut *buffer);
        }

        match job.rx.try_recv() {
            Ok(Ok(lines)) => {
                outcome = Some(JobOutcome::Finished {
                    lines,
                    elapsed: job.started_at.elapsed(),
                });
            }
            Ok(Err(e)) => {
                outcome = Some(JobOutcome::Failed {
                    // Display, not Debug: PakOperationFailed already embeds operation/context/source.
                    message: format!("{e}"),
                    elapsed: job.started_at.elapsed(),
                });
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            Err(TryRecvError::Disconnected) => {
                outcome = Some(JobOutcome::Failed {
                    message: "generation thread exited without sending a result (panicked?)".to_string(),
                    elapsed: job.started_at.elapsed(),
                });
            }
        }

        for line in drained {
            self.push_message(&line);
        }

        let Some(outcome) = outcome else {
            return;
        };
        self.pending = None;
        match outcome {
            JobOutcome::Finished { lines, elapsed } => {
                for message in lines {
                    self.push_message(&message);
                }
                self.push_message(&format!("in {:.2} seconds", elapsed.as_secs_f64()));
            }
            JobOutcome::Failed { message, elapsed } => {
                self.push_message(&message);
                self.push_message(&format!("failed after {:.2} seconds", elapsed.as_secs_f64()));
            }
        }
    }

    fn start_generation(&mut self) {
        if self.pending.is_some() {
            return;
        }

        self.push_message("Starting generation process...");

        let log: LogBuffer = Arc::new(Mutex::new(Vec::new()));
        let sink = LogSink::new({
            let log = Arc::clone(&log);
            move |line| {
                if let Ok(mut buffer) = log.lock() {
                    buffer.push(line);
                }
            }
        });

        let config = self.config.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            // A closed receiver means the app is gone; ignoring the error is intentional.
            let _ = tx.send(run_generation(config, sink));
        });

        self.pending = Some(PendingJob {
            rx,
            log,
            started_at: Instant::now(),
        });
    }
}

/// Runs the whole pipeline off the UI thread.
fn run_generation(config: AppConfig, log_sink: LogSink) -> GenerationResult {
    let mut generator = BilingualGenerator::from_config(&config)?;
    generator.set_log_sink(log_sink);
    let bilingual_set = config.pairs();
    generator.read_xml_from_paks()?;

    bilingual_set
        .par_iter()
        .map(|(primary_language, secondary_language)| {
            generator
                .process_single_bilingual(primary_language, secondary_language)
                .map(|_| format!("primary_language = {}, secondary_language = {}", primary_language, secondary_language))
        })
        .collect::<Result<Vec<String>, _>>()
}

/// egui bundles no CJK glyphs, so Chinese text — for example the localised `io::Error` messages
/// Windows produces for "access denied" — renders as tofu boxes. Borrow a system font as a
/// fallback so the messages area stays readable; Latin text keeps using Times New Roman.
fn install_cjk_fallback(fonts: &mut egui::FontDefinitions) {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string());
    let font_dir = std::path::Path::new(&windir).join("Fonts");

    // Plain .ttf first: a TrueType collection would need a face index.
    for name in ["Deng.ttf", "msyh.ttc", "SimsunExtG.ttf"] {
        let Ok(bytes) = std::fs::read(font_dir.join(name)) else {
            continue;
        };
        fonts.font_data.insert("cjk-fallback".to_owned(), egui::FontData::from_owned(bytes).into());
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            // Appended, not inserted: only characters missing from Times New Roman fall through.
            fonts.families.entry(family).or_default().push("cjk-fallback".to_owned());
        }
        return;
    }
}

/// Fonts and style are expensive to apply (they rebuild the font atlas), so this must not run per frame.
fn configure_style(ctx: &egui::Context) {
    // Load and apply Times New Roman font
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Times New Roman".to_owned(),
        egui::FontData::from_static(include_bytes!("../../../assets/times new roman.ttf")).into(),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().insert(0, "Times New Roman".to_owned());
    }
    install_cjk_fallback(&mut fonts);
    ctx.set_fonts(fonts);

    // Customize text sizes, spacing and visuals
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (egui::TextStyle::Heading, egui::FontId::new(24.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Body, egui::FontId::new(18.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Button, egui::FontId::new(18.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Small, egui::FontId::new(14.0, egui::FontFamily::Proportional)),
        // Replacing the whole map would drop Monospace, and `RichText::monospace()` panics when
        // the style is missing (it is used for the config list and the messages area).
        (egui::TextStyle::Monospace, egui::FontId::new(14.0, egui::FontFamily::Monospace)),
    ]
    .into();
    style.spacing.interact_size = egui::Vec2::new(150.0, 40.0);
    style.visuals = egui::Visuals::dark();

    // Apply to both themes so a system light/dark switch cannot wipe the customisations,
    // then pin the preference to dark instead of following the OS.
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
    ctx.set_theme(egui::ThemePreference::Dark);
}

impl eframe::App for GuiBilingualPakGenerator {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_job(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            // Window title
            ui.heading("Kingdom Come: Deliverance Bilingual Pak Generator");
            ui.separator();

            // Game Location Section
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Game Location").size(18.0));
                ui.horizontal(|ui| {
                    // The text edit needs an owned buffer; the config is only touched on change.
                    let mut path_str = self.config.game_path.clone();

                    // Create the text edit with the string copy
                    let text_edit = egui::TextEdit::singleline(&mut path_str)
                        .desired_width(ui.available_width() - 155.0)
                        .desired_rows(2)
                        .hint_text(r"E:\SteamLibrary\steamapps\common\KingdomComeDeliverance2");

                    // Always update from text input, regardless of path validity
                    let response = ui.add(text_edit);
                    if response.changed() {
                        self.set_game_path(path_str);
                    }

                    // Change Location button
                    if ui.button("Change Location").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.set_game_path(path.display().to_string());
                        }
                    }

                    // Optional: Show validation status
                    if response.lost_focus() && !self.config.game_path_buf().exists() {
                        ui.colored_label(egui::Color32::RED, "⛔ Invalid path");
                    }
                });
            });
            ui.separator();

            // Config file section: the JSON is the only setup file, so show where it lives and
            // let the pair list be edited in place. Every valid change is written straight back.
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Config").size(18.0));
                    ui.label(egui::RichText::new(self.config_path.display().to_string()).weak().size(14.0));
                    if ui
                        .button("Reload")
                        .on_hover_text("Re-read the JSON after editing it in a text editor")
                        .clicked()
                    {
                        self.reload_config();
                    }
                });

                // Restored sub-heading: names the editable block. The pair count is deliberately
                // not repeated here — the box itself shows every pair, and the count is already
                // reported in Messages.
                ui.label(egui::RichText::new("Bilingual set").size(18.0));

                // Editable list: one `Primary + Secondary` per line, comments and blank lines ok.
                // The frame is drawn explicitly: the theme's own text-edit frame is invisible on
                // this background, and an editable block must read as a field, not as a label.
                egui::ScrollArea::vertical().id_salt("bilingual_set_list").max_height(160.0).show(ui, |ui| {
                    egui::Frame::new()
                        .fill(ui.visuals().extreme_bg_color)
                        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_gray(76)))
                        .corner_radius(3)
                        .inner_margin(egui::Margin::symmetric(6, 4))
                        .show(ui, |ui| {
                            let editor = ui
                                .add(
                                    egui::TextEdit::multiline(&mut self.bilingual_set_text)
                                        .font(egui::FontId::new(14.0, egui::FontFamily::Monospace))
                                        .frame(false)
                                        .desired_rows(7)
                                        .desired_width(f32::INFINITY)
                                        .hint_text("Chineses + English"),
                                )
                                .on_hover_text("One pair per line: Primary + Secondary\nEvery edit is saved to the JSON immediately");
                            if editor.changed() {
                                self.apply_pairs_text();
                            }
                        });
                });

                if let Some(error) = &self.pairs_error {
                    ui.colored_label(egui::Color32::from_rgb(230, 120, 90), format!("⛔ {error} (the list is unchanged)"));
                }
            });
            ui.separator();

            // Generate Button
            ui.vertical(|ui| {
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(ui.available_size(), egui::Layout::top_down(egui::Align::Center), |ui| {
                        let running = self.pending.is_some();
                        // A list that does not parse would silently fall back to the previous
                        // pairs, so generation stays disabled until the box is fixed.
                        let button = ui.add_enabled(!running && self.pairs_error.is_none(), egui::Button::new("Generate Bilingual Pak"));

                        if button.clicked() {
                            self.start_generation();
                        }

                        if let Some(job) = &self.pending {
                            ui.label(format!("Generating... {:.1}s", job.started_at.elapsed().as_secs_f64()));
                        }
                    });
                });
                ui.add_space(20.0);
            });
            ui.separator();

            // Messages Area
            ui.vertical(|ui| {
                ui.label("Messages:");
                // Taller than before: warnings add several lines per run and must not be cut off.
                egui::ScrollArea::vertical().max_height(260.0).stick_to_bottom(true).show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.messages)
                            .font(egui::FontId::new(14.0, egui::FontFamily::Monospace))
                            .desired_width(f32::INFINITY),
                    );
                });
            });
        });
    }
}

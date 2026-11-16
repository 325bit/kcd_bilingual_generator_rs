use eframe::egui;
use path_finder::PathFinder;
use rayon::prelude::*;
use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, TryRecvError, channel},
    time::{Duration, Instant},
};

use generator_core::{bilingual_generator::BilingualGenerator, bilingual_generator_errors::BilingualGeneratorError};

type GenerationResult = Result<Vec<String>, BilingualGeneratorError>;

pub struct GuiBilingualPakGenerator {
    game_location: PathBuf,
    messages: String,
    /// The generation job currently running in the background, if any.
    pending: Option<PendingJob>,
}

/// Handle on a background generation job. Dropping the receiver is harmless:
/// the worker only ignores the send error and exits.
struct PendingJob {
    rx: Receiver<GenerationResult>,
    started_at: Instant,
}

/// Outcome of a finished background job, resolved once per frame.
enum JobOutcome {
    Finished { lines: Vec<String>, elapsed: Duration },
    Failed { message: String, elapsed: Duration },
}

impl Default for GuiBilingualPakGenerator {
    fn default() -> Self {
        let mut pathfinder = PathFinder::new();
        let game_location = pathfinder.find_game_path().cloned().unwrap_or_default();
        Self {
            game_location,
            messages: String::new(),
            pending: None,
        }
    }
}

impl GuiBilingualPakGenerator {
    /// Builds the app and applies the custom fonts/style exactly once.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_style(&cc.egui_ctx);
        Self::default()
    }

    /// Non-blocking result check for the background job. Requests a repaint while it is running
    /// so the elapsed time keeps ticking even without user input.
    fn poll_job(&mut self, ctx: &egui::Context) {
        let outcome = match self.pending.as_ref() {
            None => return,
            Some(job) => match job.rx.try_recv() {
                Ok(Ok(lines)) => JobOutcome::Finished {
                    lines,
                    elapsed: job.started_at.elapsed(),
                },
                Ok(Err(e)) => JobOutcome::Failed {
                    message: format!("{e:?}"),
                    elapsed: job.started_at.elapsed(),
                },
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100));
                    return;
                }
                Err(TryRecvError::Disconnected) => JobOutcome::Failed {
                    message: "generation thread exited without sending a result (panicked?)".to_string(),
                    elapsed: job.started_at.elapsed(),
                },
            },
        };

        self.pending = None;
        match outcome {
            JobOutcome::Finished { lines, elapsed } => {
                for message in lines {
                    self.messages.push_str(&message);
                    self.messages.push('\n');
                }
                self.messages.push_str(&format!("in {:.2} seconds\n", elapsed.as_secs_f64()));
            }
            JobOutcome::Failed { message, elapsed } => {
                self.messages.push_str(&format!("{message}\n"));
                self.messages.push_str(&format!("failed after {:.2} seconds\n", elapsed.as_secs_f64()));
            }
        }
    }

    fn start_generation(&mut self) {
        if self.pending.is_some() {
            return;
        }

        self.messages.push_str("Starting generation process...\n");

        let game_location = self.game_location.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            // A closed receiver means the app is gone; ignoring the error is intentional.
            let _ = tx.send(run_generation(game_location));
        });

        self.pending = Some(PendingJob {
            rx,
            started_at: Instant::now(),
        });
    }
}

/// Runs the whole pipeline off the UI thread.
fn run_generation(game_location: PathBuf) -> GenerationResult {
    let mut generator = BilingualGenerator::init()?;
    generator.game_path = game_location;
    let bilingual_set = generator.acquire_bilingual_set()?;
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
    ctx.set_fonts(fonts);

    // Customize text sizes, spacing and visuals
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (egui::TextStyle::Heading, egui::FontId::new(24.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Body, egui::FontId::new(18.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Button, egui::FontId::new(18.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Small, egui::FontId::new(14.0, egui::FontFamily::Proportional)),
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
                    // Convert PathBuf to String for editing
                    let mut path_str = self.game_location.to_string_lossy().into_owned();

                    // Create the text edit with the string copy
                    let text_edit = egui::TextEdit::singleline(&mut path_str)
                        .desired_width(ui.available_width() - 155.0)
                        .desired_rows(2);

                    // Always update from text input, regardless of path validity
                    let response = ui.add(text_edit);
                    if response.changed() {
                        self.game_location = PathBuf::from(&path_str);
                    }

                    // Change Location button
                    if ui.button("Change Location").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.game_location = path;
                        }
                    }

                    // Optional: Show validation status
                    if response.lost_focus() && !self.game_location.exists() {
                        ui.colored_label(egui::Color32::RED, "⛔ Invalid path");
                    }
                });
            });
            ui.separator();

            // Generate Button
            ui.vertical(|ui| {
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(ui.available_size(), egui::Layout::top_down(egui::Align::Center), |ui| {
                        let running = self.pending.is_some();
                        let button = ui.add_enabled(!running, egui::Button::new("Generate Bilingual Pak"));

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
                egui::ScrollArea::vertical().max_height(150.0).stick_to_bottom(true).show(ui, |ui| {
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

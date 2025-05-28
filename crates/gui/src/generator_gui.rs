use eframe::egui;
use path_finder::PathFinder;
use rayon::prelude::*;
use std::{path::PathBuf, time::Instant};

use generator_core::{bilingual_generator::BilingualGenerator, bilingual_generator_errors::BilingualGeneratorError};
pub struct GuiBilingualPakGenerator {
    game_location: PathBuf,
    messages: String,
}

impl Default for GuiBilingualPakGenerator {
    fn default() -> Self {
        let mut pathfinder = PathFinder::new();
        let kcd_path_buf = pathfinder.find_game_path().cloned().unwrap_or_default();
        Self {
            game_location: kcd_path_buf,
            messages: String::new(),
        }
    }
}

impl eframe::App for GuiBilingualPakGenerator {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Set dark theme
            ctx.set_visuals(egui::Visuals::dark());

            // Load and apply Times New Roman font
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "Times New Roman".to_owned(),
                egui::FontData::from_static(include_bytes!("../../../assets/times new roman.ttf")).into(),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "Times New Roman".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "Times New Roman".to_owned());
            ctx.set_fonts(fonts);

            // Customize text sizes and styles
            let mut style = (*ctx.style()).clone();

            // Adjust general text sizes
            style.text_styles = [
                (egui::TextStyle::Heading, egui::FontId::new(24.0, egui::FontFamily::Proportional)),
                (egui::TextStyle::Body, egui::FontId::new(18.0, egui::FontFamily::Proportional)),
                (egui::TextStyle::Button, egui::FontId::new(18.0, egui::FontFamily::Proportional)),
                (egui::TextStyle::Small, egui::FontId::new(14.0, egui::FontFamily::Proportional)),
            ]
            .into();
            style.spacing.interact_size = egui::Vec2::new(150.0, 40.0);

            // Apply custom style
            ctx.set_style(style);
            // Window title
            ui.heading("Kingdom Come: Deliverance Bilingual Pak Generator");
            ui.separator();

            // Game Location Section
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Game Location").size(18.0));
                ui.horizontal(|ui| {
                    // Convert PathBuf to String for editing
                    let mut _path_str = self.game_location.to_string_lossy().into_owned();

                    // Create the text edit with the string copy
                    let text_edit = egui::TextEdit::singleline(&mut _path_str)
                        .desired_width(ui.available_width() - 155.0)
                        .desired_rows(2);

                    // Always update from text input, regardless of path validity
                    let response = ui.add(text_edit);
                    if response.changed() {
                        self.game_location = PathBuf::from(&_path_str);
                    }

                    // Change Location button
                    if ui.button("Change Location").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.game_location = path;
                            _path_str = self.game_location.to_string_lossy().into_owned();
                            // Sync text input
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
                        if ui.button("Generate Bilingual Pak").clicked() {
                            self.messages.push_str("Starting generation process...\n");
                            let start_time = Instant::now();

                            match self.generate_bilingual_resources() {
                                Ok(generator_result_set) => {
                                    let duration = start_time.elapsed();

                                    for message in generator_result_set {
                                        self.messages.push_str(&message);
                                        self.messages.push('\n'); // Better to use single character push
                                    }

                                    // Format the duration with 2 decimal places
                                    self.messages.push_str(&format!("in {:.2} seconds", duration.as_secs_f64()));
                                }
                                Err(e) => {
                                    // println!("{}", e);
                                    self.messages.push_str(&format!("{:?}", e));
                                }
                            }
                        }
                    });
                });
                ui.add_space(20.0);
            });
            ui.separator();

            // Messages Area
            ui.vertical(|ui| {
                ui.label("Messages:");
                egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
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

impl GuiBilingualPakGenerator {
    fn generate_bilingual_resources(&mut self) -> Result<Vec<String>, BilingualGeneratorError> {
        let mut generator = BilingualGenerator::init()?;
        generator.game_path = self.game_location.clone();
        let bilingual_set = generator.acquire_bilingual_set()?;
        generator.read_xml_from_paks()?;

        let messages: Vec<String> = bilingual_set
            .par_iter()
            .map(|(primary_language, secondary_language)| {
                // Perform processing first
                let result = generator.process_single_bilingual(primary_language, secondary_language);

                // Then create message (after potential error)
                result.map(|_| format!("primary_language = {}, secondary_language = {}", primary_language, secondary_language))
            })
            .collect::<Result<Vec<String>, _>>()?;

        Ok(messages)
    }
}

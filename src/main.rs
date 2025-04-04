use gui::generator_gui::GuiBilingualPakGenerator;

mod core;
mod gui;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Bilingual Pak Generator",
        options,
        Box::new(|_cc| Ok(Box::new(GuiBilingualPakGenerator::default()))),
    )
}

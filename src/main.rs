use gui::generator_gui::GuiBilingualPakGenerator;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Bilingual Pak Generator",
        options,
        Box::new(|_cc| Ok(Box::new(GuiBilingualPakGenerator::default()))),
    )
}

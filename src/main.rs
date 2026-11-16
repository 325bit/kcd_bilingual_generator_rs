// Release builds are GUI-only: attaching to the console would show a stray cmd window.
// Debug builds keep the console so panics and eprintln! diagnostics stay visible.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use gui::generator_gui::GuiBilingualPakGenerator;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Bilingual Pak Generator",
        options,
        Box::new(|cc| Ok(Box::new(GuiBilingualPakGenerator::new(cc)))),
    )
}

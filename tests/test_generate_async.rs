// src/generate_bilingual_resources_async_test.rs
// (Or place this module content within your existing test file structure if preferred)

#[cfg(test)]
mod tests {
    use kcd_bilingual_generator_rust::core::{
        bilingual_generator::{BilingualGenerator /*, EntryId, Language, XmlFile */},
        bilingual_generator_errors::BilingualGeneratorError,
    };
    // use std::fs; // For potential cleanup
    // use std::path::PathBuf;

    #[tokio::test]
    //cargo test --release --package kcd_bilingual_generator_rust --test test_generate_async -- tests::test_generate_async --exact --show-output
    async fn test_generate_async() -> Result<(), BilingualGeneratorError> {
        println!("--- Starting test_generate_bilingual_resources_async ---");

        // 1. Initialize the generator
        // This part is synchronous
        println!("Initializing BilingualGenerator...");
        let mut generator = BilingualGenerator::init()?;
        println!("Generator initialized. Working directory: {}", generator.working_dir.display());

        // Define the expected output directory for potential cleanup
        // let output_dir = generator.working_dir.join("bilingual_xml");

        // --- Optional: Pre-test cleanup ---
        // If previous test runs left artifacts, clean them up for a fresh start.
        // if output_dir.exists() {
        //     println!("Pre-test cleanup: Removing existing directory: {}", output_dir.display());
        //     if let Err(e) = fs::remove_dir_all(&output_dir) {
        //         // Log warning but don't necessarily fail the test here
        //         eprintln!("Warning: Failed to remove pre-existing output directory: {}", e);
        //     }
        // }

        // 2. Call the asynchronous function
        // The `generate_bilingual_resources_async` function now handles
        // calling `acquire_bilingual_set` internally.
        println!("Calling generate_bilingual_resources_async()...");
        let result = generator.generate_bilingual_resources_async().await;
        println!("generate_bilingual_resources_async() completed.");

        // 3. Assert the result
        match result {
            Ok(messages) => {
                println!("Async generation successful!");
                println!("Generated messages: {:?}", messages.iter().collect::<Vec<_>>());
                println!("--- Test Passed ---");
                Ok(()) // Indicate test success
            }
            Err(e) => {
                eprintln!("Async generation failed: {:?}", e);
                println!("--- Test Failed ---");
                Err(e) // Propagate the error to fail the test
            }
        }

        // --- Optional: Post-test cleanup ---
        // Clean up generated files/directories after the test run.
        // Note: The function itself tries to clean up intermediate XMLs,
        // but it leaves the PAKs and directories.
        // if output_dir.exists() {
        //     println!("Post-test cleanup: Removing directory: {}", output_dir.display());
        //     if let Err(e) = fs::remove_dir_all(&output_dir) {
        //         eprintln!("Warning: Failed to remove output directory post-test: {}", e);
        //     }
        // }
        // If the test failed above, this cleanup might not run if you return early.
        // Consider using a scope guard (like `scopeguard::defer!`) for guaranteed cleanup
        // if that's critical, even on panic or error.
    }
}

/* ------------------------------------------------------------------------------------------------------------------------------- */
// Processor:       Intel 12th Generation Core i5-12400F Six Core
// Motherboard:     Mingzhang MS-H610M 666 WIFI6 (Intel H610 chipset)
// Memory:          Yuzhan 16GB DDR4 3200MHz (8GB+8GB)
// Gpu:             NVIDIA GeForce RTX 4060 (8GB/Colorful)
// Disk:            Predator SSD GM7 M.2 2TB(2048GB)
/* ----------------------------------------------------------test result (dev mode)---------------------------------------------------------- */
// test tests::test_generate_bilingual_resources_async ... ok

// successes:

// ---- tests::test_generate_bilingual_resources_async stdout ----
// --- Starting test_generate_bilingual_resources_async ---
// Initializing BilingualGenerator...
// Generator initialized. Working directory: D:\FileSaves\code\rust\kcd_bilingual_generator_rs
// Pre-test cleanup: Removing existing directory: D:\FileSaves\code\rust\kcd_bilingual_generator_rs\bilingual_xml
// Calling generate_bilingual_resources_async()...
// Required languages to read: ["Chineset", "Spanish", "Czech", "Japanese", "Chineses", "English", "French", "German"]
// All reader tasks spawned.
// Coordinator started. Waiting for language data...
// [Reader: Chineset] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\Chineset_xml.pak
// [Reader: Spanish] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\Spanish_xml.pak
// [Reader: Czech] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\Czech_xml.pak
// [Reader: Japanese] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\Japanese_xml.pak
// [Reader: Chineses] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\Chineses_xml.pak
// [Reader: English] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\English_xml.pak
// [Reader: French] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\French_xml.pak
// [Reader: German] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\German_xml.pak
// [Reader: Czech] Successfully finished reading all files.
// Coordinator: Received data for language: Czech
// [Reader: English] Successfully finished reading all files.
// Coordinator: Received data for language: English
// [Reader: Chineses] Successfully finished reading all files.
// Coordinator: Received data for language: Chineses
// Coordinator: Data ready for pair: Chineses + English. Spawning processor.
// Coordinator: Data ready for pair: Chineses + Czech. Spawning processor.
// [Processor: Chineses + English] Starting processing.
// [Processor: Chineses + Czech] Starting processing.
// [Reader: Spanish] Successfully finished reading all files.
// Coordinator: Received data for language: Spanish
// Coordinator: Data ready for pair: Chineses + Spanish. Spawning processor.
// [Processor: Chineses + Spanish] Starting processing.
// [Reader: German] Successfully finished reading all files.
// Coordinator: Received data for language: German
// Coordinator: Data ready for pair: Chineses + German. Spawning processor.
// [Processor: Chineses + German] Starting processing.
// [Reader: Chineset] Successfully finished reading all files.
// Coordinator: Received data for language: Chineset
// Coordinator: Data ready for pair: Chineset + English. Spawning processor.
// [Processor: Chineset + English] Starting processing.
// [Reader: French] Successfully finished reading all files.
// Coordinator: Received data for language: French
// Coordinator: Data ready for pair: Chineses + French. Spawning processor.
// [Processor: Chineses + French] Starting processing.
// [Reader: Japanese] Successfully finished reading all files.
// Coordinator: Received data for language: Japanese
// Coordinator: Data ready for pair: Chineses + Japanese. Spawning processor.
// Coordinator: All language data received or reader tasks finished.
// Coordinator: Waiting for 7 processing tasks to complete...
// [Processor: Chineses + Japanese] Starting processing.
// [Processor: Chineses + Spanish] Creating PAK file.
// [Processor: Chineses + Czech] Creating PAK file.
// [Processor: Chineses + English] Creating PAK file.
// [Processor: Chineses + German] Creating PAK file.
// [Processor: Chineset + English] Creating PAK file.
// [Processor: Chineses + French] Creating PAK file.
// [Processor: Chineses + Japanese] Creating PAK file.
// [Processor: Chineset + English] PAK created successfully. Cleaning up XML files.
// [Processor: Chineset + English] Finished processing.
// [Processor: Chineses + Czech] PAK created successfully. Cleaning up XML files.
// [Processor: Chineses + Czech] Finished processing.
// [Processor: Chineses + Spanish] PAK created successfully. Cleaning up XML files.
// [Processor: Chineses + Spanish] Finished processing.
// [Processor: Chineses + English] PAK created successfully. Cleaning up XML files.
// [Processor: Chineses + English] Finished processing.
// [Processor: Chineses + French] PAK created successfully. Cleaning up XML files.
// [Processor: Chineses + French] Finished processing.
// [Processor: Chineses + German] PAK created successfully. Cleaning up XML files.
// [Processor: Chineses + German] Finished processing.
// [Processor: Chineses + Japanese] PAK created successfully. Cleaning up XML files.
// [Processor: Chineses + Japanese] Finished processing.
// Coordinator: All processing tasks finished.
// Coordinator: Finished successfully.
// generate_bilingual_resources_async() completed.
// Async generation successful!
// Generated messages: ["primary_language = Chineses, secondary_language = English", "primary_language = Chineses, secondary_language = Czech", "primary_language = Chineses, secondary_language
// = Spanish", "primary_language = Chineses, secondary_language = German", "primary_language = Chineset, secondary_language = English", "primary_language = Chineses, secondary_language = French", "primary_language = Chineses, secondary_language = Japanese"]
// --- Test Passed ---

// successes:
//     tests::test_generate_bilingual_resources_async

// test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.25s

// after applying faststr
// --- Performance Summary (10 runs) ---
// Individual run durations:
//   Run 1: 10.775803s
//   Run 2: 10.5967178s
//   Run 3: 10.4683726s
//   Run 4: 10.35491s
//   Run 5: 10.5411002s
//   Run 6: 10.4720224s
//   Run 7: 10.5750342s
//   Run 8: 11.0741312s
//   Run 9: 10.747729s
//   Run 10: 10.432761s
// Total time for 10 runs: 106.0385814s
// Average run time: 10.60385814s
/* ----------------------------------------------------------test result (release mode, lto = thin)---------------------------------------------------------- */
// test tests::test_generate_bilingual_resources_async ... ok

// successes:

// ---- tests::test_generate_bilingual_resources_async stdout ----
// --- Starting test_generate_bilingual_resources_async ---
// Initializing BilingualGenerator...
// Generator initialized. Working directory: D:\FileSaves\code\rust\kcd_bilingual_generator_rs
// Calling generate_bilingual_resources_async()...
// Required languages to read: ["English", "Chineses", "Chineset", "Japanese", "German", "Spanish", "Czech", "French"]
// All reader tasks spawned.
// Coordinator started. Waiting for language data...
// [Reader: English] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\English_xml.pak
// [Reader: Chineses] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\Chineses_xml.pak
// [Reader: Chineset] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\Chineset_xml.pak
// [Reader: Japanese] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\Japanese_xml.pak
// [Reader: German] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\German_xml.pak
// [Reader: Spanish] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\Spanish_xml.pak
// [Reader: Czech] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\Czech_xml.pak
// [Reader: French] Opening PAK: E:\\SteamLibrary\steamapps\common\KingdomComeDeliverance2\Localization\French_xml.pak
// [Reader: English] Successfully finished reading all files.
// Coordinator: Received data for language: English
// [Reader: Spanish] Successfully finished reading all files.
// Coordinator: Received data for language: Spanish
// [Reader: Chineset] Successfully finished reading all files.
// Coordinator: Received data for language: Chineset
// Coordinator: Data ready for pair: Chineset + English. Spawning processor.
// [Processor: Chineset + English] Starting processing.
// [Reader: French] Successfully finished reading all files.
// [Reader: German] Successfully finished reading all files.
// Coordinator: Received data for language: French
// Coordinator: Received data for language: German
// [Reader: Czech] Successfully finished reading all files.
// Coordinator: Received data for language: Czech
// [Reader: Chineses] Successfully finished reading all files.
// Coordinator: Received data for language: Chineses
// Coordinator: Data ready for pair: Chineses + English. Spawning processor.
// Coordinator: Data ready for pair: Chineses + French. Spawning processor.
// [Processor: Chineses + English] Starting processing.
// Coordinator: Data ready for pair: Chineses + German. Spawning processor.
// Coordinator: Data ready for pair: Chineses + Spanish. Spawning processor.
// Coordinator: Data ready for pair: Chineses + Czech. Spawning processor.
// [Processor: Chineses + French] Starting processing.
// [Processor: Chineses + German] Starting processing.
// [Processor: Chineses + Spanish] Starting processing.
// [Processor: Chineses + Czech] Starting processing.
// [Reader: Japanese] Successfully finished reading all files.
// Coordinator: Received data for language: Japanese
// Coordinator: Data ready for pair: Chineses + Japanese. Spawning processor.
// Coordinator: All language data received or reader tasks finished.
// Coordinator: Waiting for 7 processing tasks to complete...
// [Processor: Chineses + Japanese] Starting processing.
// [Processor: Chineses + Spanish] Creating PAK file.
// [Processor: Chineset + English] Creating PAK file.
// [Processor: Chineses + German] Creating PAK file.
// [Processor: Chineses + Czech] Creating PAK file.
// [Processor: Chineses + English] Creating PAK file.
// [Processor: Chineses + Japanese] Creating PAK file.
// [Processor: Chineses + French] Creating PAK file.
// [Processor: Chineses + Spanish] PAK created successfully. Cleaning up XML files.
// [Processor: Chineses + Spanish] Finished processing.
// [Processor: Chineses + German] PAK created successfully. Cleaning up XML files.
// [Processor: Chineses + German] Finished processing.
// [Processor: Chineses + English] PAK created successfully. Cleaning up XML files.
// [Processor: Chineses + English] Finished processing.
// [Processor: Chineses + Czech] PAK created successfully. Cleaning up XML files.
// [Processor: Chineses + Czech] Finished processing.
// [Processor: Chineses + French] PAK created successfully. Cleaning up XML files.
// [Processor: Chineset + English] PAK created successfully. Cleaning up XML files.
// [Processor: Chineset + English] Finished processing.
// [Processor: Chineses + French] Finished processing.
// [Processor: Chineses + Japanese] PAK created successfully. Cleaning up XML files.
// [Processor: Chineses + Japanese] Finished processing.
// Coordinator: All processing tasks finished.
// Coordinator: Finished successfully.
// generate_bilingual_resources_async() completed.
// Async generation successful!
// Generated messages: ["primary_language = Chineset, secondary_language = English", "primary_language = Chineses, secondary_language = English", "primary_language = Chineses, secondary_language = French", "primary_language = Chineses, secondary_language = German", "primary_language = Chineses, secondary_language = Spanish", "primary_language = Chineses, secondary_language = Czech", "primary_language = Chineses, secondary_language = Japanese"]
// --- Test Passed ---

// successes:
//     tests::test_generate_bilingual_resources_async

// test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.23s

// after applying faststr
// --- Performance Summary ---
// Successful runs: 10 / 10
// Individual successful run times (seconds reported by Cargo):
//   2.140
//   2.170
//   2.160
//   2.140
//   2.190
//   2.220
//   2.190
//   2.160
//   2.140
//   2.150
// Total time reported by Cargo across successful runs: 21.660s
// Average time reported by Cargo per successful run: 2.166s

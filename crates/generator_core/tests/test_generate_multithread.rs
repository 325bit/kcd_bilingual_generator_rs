#[cfg(test)]
mod tests {
    use generator_core::{
        bilingual_generator::{BilingualGenerator /*, EntryId, Language, XmlFile */},
        bilingual_generator_errors::BilingualGeneratorError,
    };
    use rayon::prelude::*;

    #[test]
    //cargo test --release --package generator_core --test test_generate_multithread -- tests::test_generate_multithread --exact --show-output
    fn test_generate_multithread() -> Result<(), BilingualGeneratorError> {
        let mut generator = BilingualGenerator::init()?;
        let bilingual_set = generator.acquire_bilingual_set()?;

        generator.read_xml_from_paks()?;

        bilingual_set.par_iter().try_for_each(|(primary_language, secondary_language)| {
            println!("primary_language = {}, secondary_language = {}", primary_language, secondary_language);
            generator.process_single_bilingual(primary_language, secondary_language).map(|_| ())
        })?;

        Ok(())
    }
}

/* ------------------------------------------------------------------------------------------------------------------------------- */
// Processor:       Intel 12th Generation Core i5-12400F Six Core
// Motherboard:     Mingzhang MS-H610M 666 WIFI6 (Intel H610 chipset)
// Memory:          Yuzhan 16GB DDR4 3200MHz (8GB+8GB)
// Gpu:             NVIDIA GeForce RTX 4060 (8GB/Colorful)
// Disk:            Predator SSD GM7 M.2 2TB(2048GB)
/* ----------------------------------------------------------test result (dev mode)---------------------------------------------------------- */
// test tests::generate_bilingual_resources_multithread_test ... ok

// successes:

// ---- tests::generate_bilingual_resources_multithread_test stdout ----
// primary_language = Chineset, secondary_language = English
// primary_language = Chineses, secondary_language = English
// primary_language = Chineses, secondary_language = Spanish
// primary_language = Chineses, secondary_language = French
// primary_language = Chineses, secondary_language = Japanese
// primary_language = Chineses, secondary_language = German
// primary_language = Chineses, secondary_language = Czech

// successes:
//     tests::generate_bilingual_resources_multithread_test

// test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.36s
/* ----------------------------------------------------------test result (release mode, lto = thin)---------------------------------------------------------- */
// test tests::generate_bilingual_resources_multithread_test ... ok

// successes:

// ---- tests::generate_bilingual_resources_multithread_test stdout ----
// primary_language = Chineses, secondary_language = Japanese
// primary_language = Chineset, secondary_language = English
// primary_language = Chineses, secondary_language = French
// primary_language = Chineses, secondary_language = Spanish
// primary_language = Chineses, secondary_language = English
// primary_language = Chineses, secondary_language = Czech
// primary_language = Chineses, secondary_language = German

// successes:
//     tests::generate_bilingual_resources_multithread_test

// test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.20s
// --- Performance Summary ---
// Successful runs: 10 / 10
// Individual successful run times (seconds reported by Cargo):
//   2.170
//   2.120
//   2.130
//   2.170
//   2.100
//   2.110
//   2.150
//   2.170
//   2.160
//   2.170
// Total time reported by Cargo across successful runs: 21.450s
// Average time reported by Cargo per successful run: 2.145s

/* ----------------------------------------------------------test result (release mode, With SQL)---------------------------------------------------------- */
// successes:
//     tests::test_generate_multithread

// test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 28.84s

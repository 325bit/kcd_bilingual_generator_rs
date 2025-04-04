#[cfg(test)]
mod tests {
    use kcd_bilingual_generator_rust::core::{
        bilingual_generator::{BilingualGenerator /*, EntryId, Language, XmlFile */},
        bilingual_generator_errors::BilingualGeneratorError,
    };
    use rayon::prelude::*;

    #[test]
    fn generate_bilingual_resources_multithread_test() -> Result<(), BilingualGeneratorError> {
        let mut generator = BilingualGenerator::init()?; // Immutable generator
        let bilingual_set = generator.acquire_bilingual_set()?;

        generator.read_xml_from_paks()?;

        bilingual_set
            .par_iter()
            .try_for_each(|(primary_language, secondary_language)| {
                println!(
                    "primary_language = {}, secondary_language = {}",
                    primary_language, secondary_language
                );
                generator
                    .process_single_bilingual(primary_language, secondary_language)
                    .map(|_| ())
            })?;

        Ok(())
    }
}
//cargo test --release --package kcd_bilingual_generator_rust --test generate_bilingual_resources_multithread_test -- tests::generate_bilingual_resources_multithread_test --exact --show-output

/* ------------------------------------------------------------------------------------------------------------------------------- */
// Processor:       Intel 12th Generation Core i5-12400F Six Core
// Motherboard:     Mingzhang MS-H610M 666 WIFI6 (Intel H610 chipset)
// Memory:          Yuzhan 16GB DDR4 3200MHz (8GB+8GB)
// Gpu:             NVIDIA GeForce RTX 4060 (8GB/Colorful)
// Disk:            Predator SSD GM7 M.2 2TB(2048GB)
/* ----------------------------------------------------------test result (dev mode)---------------------------------------------------------- */
// test tests::generate_bilingual_resources_multithread_test ... ok

// running 1 test
// successes:

// ---- tests::generate_bilingual_resources_multithread_test stdout ----
// primary_language = Chineses, secondary_language = English
// primary_language = Chineses, secondary_language = French

// successes:
//     tests::generate_bilingual_resources_multithread_test

// test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.51s
/* ----------------------------------------------------------test result (release mode)---------------------------------------------------------- */
// test tests::generate_bilingual_resources_multithread_test ... ok

// successes:

// ---- tests::generate_bilingual_resources_multithread_test stdout ----
// primary_language = Chineses, secondary_language = English
// primary_language = Chineses, secondary_language = French

// successes:
//     tests::generate_bilingual_resources_multithread_test

// test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.54s

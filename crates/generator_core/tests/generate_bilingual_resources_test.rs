#[cfg(test)]
mod tests {
    use generator_core::{
        bilingual_generator::{BilingualGenerator /*, EntryId, Language, XmlFile */},
        bilingual_generator_errors::BilingualGeneratorError,
    };

    #[test]
    fn generate_bilingual_resources_test() -> Result<(), BilingualGeneratorError> {
        let mut generator = BilingualGenerator::init()?;
        let bilingual_set = generator.acquire_bilingual_set()?;
        match generator.read_xml_from_paks() {
            Ok(_) => {
                for (primary_language, secondary_language) in bilingual_set {
                    println!("primary_language = {}, secondary_language = {}", primary_language, secondary_language);
                    match generator.process_single_bilingual(&primary_language, &secondary_language) {
                        Ok(_) => continue,
                        Err(e) => return Err(e),
                    }
                }
                Ok(())
            }
            Err(_) => todo!(),
        }
    }
}
//cargo test --release --package kcd_bilingual_generator_rust --test generate_bilingual_resources_test -- tests::generate_bilingual_resources_test --exact --show-output

/* ------------------------------------------------------------------------------------------------------------------------------- */
// Processor:       Intel 12th Generation Core i5-12400F Six Core
// Motherboard:     Mingzhang MS-H610M 666 WIFI6 (Intel H610 chipset)
// Memory:          Yuzhan 16GB DDR4 3200MHz (8GB+8GB)
// Gpu:             NVIDIA GeForce RTX 4060 (8GB/Colorful)
// Disk:            Predator SSD GM7 M.2 2TB(2048GB)
/* ----------------------------------------------------------test result (dev mode)---------------------------------------------------------- */
// test tests::generate_bilingual_resources_test ... ok

// successes:

// ---- tests::generate_bilingual_resources_test stdout ----
// primary_language = Chineset, secondary_language = English
// primary_language = Chineses, secondary_language = English
// primary_language = Chineses, secondary_language = French
// primary_language = Chineses, secondary_language = German
// primary_language = Chineses, secondary_language = Spanish
// primary_language = Chineses, secondary_language = Czech
// primary_language = Chineses, secondary_language = Japanese

// successes:
//     tests::generate_bilingual_resources_test

// test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 37.13s
/* ----------------------------------------------------------test result (release mode)---------------------------------------------------------- */
// test tests::generate_bilingual_resources_test ... ok

// successes:

// ---- tests::generate_bilingual_resources_test stdout ----
// primary_language = Chineset, secondary_language = English
// primary_language = Chineses, secondary_language = English
// primary_language = Chineses, secondary_language = French
// primary_language = Chineses, secondary_language = German
// primary_language = Chineses, secondary_language = Spanish
// primary_language = Chineses, secondary_language = Czech
// primary_language = Chineses, secondary_language = Japanese

// successes:
//     tests::generate_bilingual_resources_test

// test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.89s

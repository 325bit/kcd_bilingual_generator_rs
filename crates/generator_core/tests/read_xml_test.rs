#[cfg(test)]
mod tests {
    // Make sure BilingualGenerator is accessible. Adjust path if needed.
    // Assuming generator_core is the crate name or a module.
    use generator_core::bilingual_generator::BilingualGenerator;
    use sqlx::Row; // Import the Row trait to get data from query results

    #[test]
    fn read_xml_and_query_db_test() -> Result<(), Box<dyn std::error::Error>> {
        // --- Setup ---
        println!("Setting up test...");

        // Initialize the generator (connects to DB, creates table if needed)
        let mut generator = BilingualGenerator::init().expect("Failed to initialize BilingualGenerator (check DB connection)");

        // --- Action ---
        // Call the read_xml_from_paks function to parse XML files and populate DB
        println!("Calling read_xml_from_paks()...");
        match generator.read_xml_from_paks() {
            Ok(_) => {
                println!("read_xml_from_paks() completed successfully.");
                // --- Verification ---
                println!("Querying database for specific entry...");

                // Define the data we want to retrieve
                let target_xml_file = "text_ui_soul.xml";
                let target_language = "Chineses";
                let target_entry_id = "buff_alcoholism_level3_desc";

                // Get the runtime and pool from the generator
                let runtime = &generator.runtime; // Borrow runtime
                let pool = &generator.db_pool; // Borrow pool

                // Execute the query using block_on
                let query_result = runtime.block_on(async {
                    sqlx::query("SELECT text_value FROM text_entries WHERE xml_file = $1 AND language = $2 AND entry_id = $3")
                        .bind(target_xml_file)
                        .bind(target_language)
                        .bind(target_entry_id)
                        .fetch_one(pool) // Expect exactly one row
                        .await
                });

                match query_result {
                    Ok(row) => {
                        // Extract the text_value (we know it's the first column, index 0, and TEXT -> String)
                        let content: String = row.try_get(0)?; // Use try_get for robust error handling
                        println!("--------------------------------------------------");
                        println!("Successfully retrieved from DB:");
                        println!("  File:     {}", target_xml_file);
                        println!("  Language: {}", target_language);
                        println!("  Entry ID: {}", target_entry_id);
                        println!("  Content:  {}", content);
                        println!("--------------------------------------------------");
                        Ok(()) // Test passed
                    }
                    Err(e) => {
                        eprintln!("Database query failed: {}", e);
                        Err(format!("Failed to fetch expected data from database: {}", e).into()) // Convert sqlx::Error to Box<dyn Error>
                    }
                }
            }
            Err(e) => {
                eprintln!("read_xml_from_paks() failed: {:?}", e);
                Err(format!("Failed during read_xml_from_paks: {:?}", e).into()) // Convert BilingualGeneratorError to Box<dyn Error>
            }
        }
    }
}

// --------------------------------------------------
// test tests::read_xml_and_query_db_test ... ok

// successes:

// ---- tests::read_xml_and_query_db_test stdout ----
// Setting up test...
// Calling read_xml_from_paks()...
// read_xml_from_paks() completed successfully.
// Querying database for specific entry...
// --------------------------------------------------
// Successfully retrieved from DB:
//   File:     text_ui_soul.xml
//   Language: Chineses
//   Entry ID: buff_alcoholism_level3_desc
//   Content:  你有酗酒的毛病。除非你的酒瘾得到满足，否则你的力量和活力会降低1点。长期戒酒可以治愈该症状。
// --------------------------------------------------

// successes:
//     tests::read_xml_and_query_db_test

// test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 28.41s

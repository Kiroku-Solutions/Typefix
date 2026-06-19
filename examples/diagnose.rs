/// Quick diagnostic: does the pipeline actually correct "teh" with real data?
use std::sync::Arc;

fn main() {
    // Init logging
    tracing_subscriber::fmt::init();

    // Step 1: Init engine with real data
    let config = typefix::core::config::Config {
        data_path: std::path::PathBuf::from("data"),
        default_language: "en".to_string(),
        supported_languages: vec!["en".to_string(), "es".to_string(), "pt".to_string()],
        user_preferred_language: Some("en".to_string()),
        ..typefix::core::config::Config::default()
    };

    typefix::init(&config).expect("init failed");

    // Step 2: Build pipeline the same way the daemon does
    let pipeline = typefix::pipeline::TypeFixPipeline::new(typefix::pipeline::PipelineConfig::default());
    {
        let state_arc = typefix::get_state();
        let state = state_arc.read();

        println!("=== ENGINE STATE ===");
        println!("Active language: {}", state.active_language);
        println!("Dictionaries loaded: {:?}", state.dictionaries.keys().collect::<Vec<_>>());
        println!("Error maps loaded: {:?}", state.error_maps.keys().collect::<Vec<_>>());
        println!("Stopwords loaded: {:?}", state.stopwords.keys().collect::<Vec<_>>());

        // Check if "teh" is in the error map
        if let Some(en_map) = state.error_maps.get("en") {
            let lookup = en_map.lookup("teh");
            println!("\nError map lookup 'teh' -> {:?}", lookup);
        } else {
            println!("\nNO error map for 'en'!");
        }

        // Check dictionary size
        if let Some(en_dict) = state.dictionaries.get("en") {
            println!("EN dictionary size: {} words", en_dict.len());
            println!("Dict contains 'the': {}", en_dict.contains("the"));
            println!("Dict contains 'teh': {}", en_dict.contains("teh"));
            let similar = en_dict.find_similar("teh", 1, 3);
            println!("find_similar('teh', 1, 3) -> {:?}", similar);
        }

        for (lang, dict) in &state.dictionaries {
            pipeline.add_dictionary(lang, Arc::clone(dict));
        }
        for (lang, sw) in &state.stopwords {
            pipeline.add_stopwords(lang, Arc::clone(sw));
        }
        for (lang, em) in &state.error_maps {
            pipeline.add_error_map(lang, Arc::clone(em));
        }
        pipeline.set_language(&state.active_language);
    }

    // Step 3: Simulate typing "teh " exactly like the daemon does
    println!("\n=== PIPELINE SIMULATION ===");
    println!("Pushing 't'...");
    let r1 = pipeline.push('t');
    println!("  Result: {:?}", r1);

    println!("Pushing 'e'...");
    let r2 = pipeline.push('e');
    println!("  Result: {:?}", r2);

    println!("Pushing 'h'...");
    let r3 = pipeline.push('h');
    println!("  Result: {:?}", r3);

    println!("Pushing ' ' (space delimiter)...");
    let r4 = pipeline.push(' ');
    println!("  Result: {:?}", r4);

    if let Some(result) = r4 {
        println!("\n=== CORRECTION RESULT ===");
        println!("Original: {}", result.original);
        println!("Corrected: {:?}", result.corrected);
    } else {
        println!("\nNO result from pipeline! The space didn't trigger extraction.");
    }
}

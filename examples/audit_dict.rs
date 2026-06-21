use std::path::Path;
use std::sync::Arc;
use typefix::core::Dict;
use typefix::correction::engine::EngineConfig;
use typefix::correction::StaticErrorMap;
use typefix::CorrectionEngine;
use typefix::LanguageDetector;

fn main() {
    println!("=== TEST: typefix real-engine diagnostic ===\n");

    // Load Spanish FST
    let es_path = Path::new("data/dictionaries/es.fst");
    let es_dict = Dict::from_fst_file(es_path).expect("load es.fst");
    println!("Loaded es.fst: {} words", es_dict.len());

    // Load Spanish error map
    let err_path = Path::new("data/errors/es.json");
    let es_errors = StaticErrorMap::from_json_file(err_path).expect("load errors/es.json");
    println!("Loaded es.json errors\n");

    let mut engine = CorrectionEngine::new(EngineConfig {
        max_edit_distance: 2,
        max_candidates: 5,
        min_word_length: 2,
        case_sensitive: false, enforce_accents: false,
    });
    engine.add_dictionary("es", Arc::new(es_dict));
    engine.add_error_map(Arc::new(es_errors), "es");

    // Set language
    let detector = Arc::new(LanguageDetector::new(Default::default()));
    detector.set_language("es");
    engine.set_detector(detector);

    let test_words = vec![
        // 1) Common words that should be IN the dictionary
        ("hola", "should be valid"),
        ("casa", "should be valid"),
        ("perro", "should be valid"),
        ("agua", "should be valid"),
        ("como", "should be valid"),
        ("que", "should be valid"),
        ("para", "should be valid"),
        // 2) Typos of common words — should be corrected
        ("qeu", "should correct to 'que'"),
        ("komo", "should correct to 'como'"),
        ("ola", "should correct to 'hola' (d=1)"),
        ("csaa", "should correct to 'casa' (d=2)"),
        ("pero", "should be valid"),
        ("perozo", "should NOT match 'pero'"),
        // 3) Spanish words with accents
        ("cómo", "should be valid"),
        ("está", "should be valid"),
        ("más", "should be valid"),
        ("tambien", "should be valid (no accent)"),
        // 4) Random gibberish
        ("xqz", "should NOT be corrected"),
        ("zxxy", "should NOT be corrected"),
    ];

    println!("{:>10} | {:>14} | {:>14} | {}", "INPUT", "STATUS", "OUTPUT", "NOTE");
    println!("{}", "-".repeat(80));
    for (word, note) in test_words {
        let result = engine.correct(word);
        let status = if result.corrected.is_some() { "CORRECTED" }
                     else if result.source == typefix::correction::engine::CorrectionSource::None { "no-fix" }
                     else { "valid" };
        let output = result.corrected.clone().unwrap_or_else(|| word.to_string());
        println!("{:>10} | {:>14} | {:>14} | {}", word, status, output, note);
    }

    // Now query the FST directly to see what's actually in it
    println!("\n=== FST contains check ===");
    let es_dict = Dict::from_fst_file(Path::new("data/dictionaries/es.fst")).unwrap();
    let queries = vec!["como", "cómo", "que", "qué", "casa", "hola", "està", "está", "mañana", "tambien", "también"];
    for w in queries {
        let contains = es_dict.contains(w);
        let freq = es_dict.search(w).unwrap_or(0);
        println!("  contains(\"{}\") = {}  freq = {}", w, contains, freq);
    }
}

use std::path::Path;
use std::sync::Arc;
use typefix::core::Dict;
use typefix::correction::engine::EngineConfig;
use typefix::correction::StaticErrorMap;
use typefix::CorrectionEngine;
use typefix::LanguageDetector;

fn build_engine(lang: &str, max_distance: usize) -> CorrectionEngine {
    let fst_path = format!("data/dictionaries/{}.fst", lang);
    let dict = Dict::from_fst_file(Path::new(&fst_path))
        .expect(&format!("load {}.fst", lang));
    println!("Loaded {}.fst: {} words", lang, dict.len());

    let err_str = format!("data/errors/{}.json", lang);
    let err_path = Path::new(&err_str);
    let err_map = StaticErrorMap::from_json_file(err_path).expect("load errors");
    let stats = err_map.stats();
    println!("Loaded {}.json errors -> static_errors={} user_errors={} lang={}",
        lang, stats.static_errors, stats.user_errors, stats.language);

    let mut engine = CorrectionEngine::new(EngineConfig {
        max_edit_distance: max_distance,
        max_candidates: 5,
        min_word_length: 2,
        case_sensitive: false,
    });
    engine.add_dictionary(lang, Arc::new(dict));
    engine.add_error_map(Arc::new(err_map), lang);

    let detector = Arc::new(LanguageDetector::new(Default::default()));
    detector.set_language(lang);
    engine.set_detector(detector);
    engine
}

fn main() {
    println!("=== AUDIT 2: User's specific words ===\n");

    println!("-- Spanish at d=2 (config.json) --");
    let engine_es = build_engine("es", 2);
    let test_words_es = vec![
        "porqeu",       // transposition: porque
        "mueotr",       // transposition: muerto? muestra? mueve?
        "no",           // valid
        "se",           // valid
        "actualidad",   // valid - but should be "actualiza"?
        "actuualizar",  // typo of actualizar
        "olaa",         // hola?
        "csaa",         // casa
        "ahorita",      // ahora (latam slang, not in dict)
        "kasa",         // casa
        "mujer",        // valid
    ];
    println!("\n{:<14} | {:<14} | {:<12} | {}", "INPUT", "STATUS", "OUTPUT", "FST-contains?");
    println!("{}", "-".repeat(70));
    for w in &test_words_es {
        let result = engine_es.correct(w);
        let status = if result.corrected.is_some() { "CORRECTED" } else { "no-fix" };
        let out = result.corrected.clone().unwrap_or_else(|| w.to_string());
        // Check FST directly (post-encoder)
        let dict_check = Dict::from_fst_file(Path::new("data/dictionaries/es.fst")).unwrap();
        let contains = dict_check.contains(w);
        let cands = engine_es.get_corrections(w);
        let cands_str = cands.iter().map(|c| format!("{}(d{},f{})", c.word, c.distance, c.frequency))
            .collect::<Vec<_>>().join(", ");
        println!("{:<14} | {:<14} | {:<12} | contains={} candidates=[{}]",
            w, status, out, contains, cands_str);
    }

    println!("\n-- Spanish FST direct probes --");
    let es = Dict::from_fst_file(Path::new("data/dictionaries/es.fst")).unwrap();
    for w in &["como", "cómo", "que", "qué", "porque", "muerto", "muestra",
               "muestreo", "muere", "actualiza", "actualidad", "porqeu", "mueotr",
               "actualizar", "mujer", "hola", "ola", "tambien", "también"] {
        let contains = es.contains(w);
        let freq = es.search(w).unwrap_or(0);
        println!("  {:<14} contains={} freq={}", w, contains, freq);
    }

    println!("\n-- English at d=2 (config.json) --");
    let engine_en = build_engine("en", 2);
    let test_words_en = vec!["recieve", "definately", "teh", "adn", "writting", "thier"];
    for w in &test_words_en {
        let result = engine_en.correct(w);
        let status = if result.corrected.is_some() { "CORRECTED" } else { "no-fix" };
        let out = result.corrected.clone().unwrap_or_else(|| w.to_string());
        let cands = engine_en.get_corrections(w);
        let cands_str = cands.iter().map(|c| format!("{}(d{},f{})", c.word, c.distance, c.frequency))
            .collect::<Vec<_>>().join(", ");
        println!("{:<14} | {:<14} | {:<12} | [{}]", w, status, out, cands_str);
    }
}

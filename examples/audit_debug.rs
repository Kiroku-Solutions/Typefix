use std::path::Path;
use std::sync::Arc;
use typefix::core::Dict;
use typefix::correction::engine::EngineConfig;
use typefix::correction::StaticErrorMap;
use typefix::CorrectionEngine;
use typefix::LanguageDetector;

fn main() {
    let dict = Dict::from_fst_file(Path::new("data/dictionaries/es.fst")).unwrap();
    let err_map = StaticErrorMap::from_json_file(Path::new("data/errors/es.json")).unwrap();

    let mut engine = CorrectionEngine::new(EngineConfig {
        max_edit_distance: 2,
        max_candidates: 5,
        min_word_length: 2,
        case_sensitive: false, enforce_accents: false,
    });
    engine.add_dictionary("es", Arc::new(dict));
    engine.add_error_map(Arc::new(err_map), "es");

    let detector = Arc::new(LanguageDetector::new(Default::default()));
    detector.set_language("es");
    engine.set_detector(detector);

    // The como -> como check
    let result = engine.correct("como");
    println!("como    -> corrected={:?} source={:?}", result.corrected, result.source);
    let cands = engine.get_corrections("como");
    for c in &cands {
        println!("  candidate: {} d={} f={}", c.word, c.distance, c.frequency);
    }

    // The ambiguity
    println!();
    let result = engine.correct("tambien");
    println!("tambien -> corrected={:?} source={:?}", result.corrected, result.source);
    let cands = engine.get_corrections("tambien");
    for c in &cands {
        println!("  candidate: {} d={} f={}", c.word, c.distance, c.frequency);
    }

    // Now check that the cross-language check is what it should be
    println!();
    let result = engine.correct("peroozo");
    println!("peroozo -> corrected={:?} source={:?}", result.corrected, result.source);
    let cands = engine.get_corrections("peroozo");
    for c in &cands {
        println!("  candidate: {} d={} f={}", c.word, c.distance, c.frequency);
    }

    // porqeu
    let result = engine.correct("porqeu");
    println!("porqeu  -> corrected={:?} source={:?}", result.corrected, result.source);
    let cands = engine.get_corrections("porqeu");
    for c in &cands {
        println!("  candidate: {} d={} f={}", c.word, c.distance, c.frequency);
    }
}

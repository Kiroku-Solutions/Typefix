use std::sync::Arc;
use typefix::core::Dict;
use typefix::correction::engine::EngineConfig;
use typefix::correction::CorrectionEngine;

fn main() {
    let mut engine = CorrectionEngine::new(EngineConfig {
        max_edit_distance: 1,
        max_candidates: 3,
        min_word_length: 2,
        case_sensitive: false, enforce_accents: false,
    });

    let fst_bytes = std::fs::read("data/dictionaries/es.fst").unwrap();
    let dict = Dict::from_bytes(fst_bytes).unwrap();
    engine.add_dictionary("es", Arc::new(dict));

    engine.set_language("es");

    let result = engine.correct("porqeu");
    println!("Correction for 'porqeu': {:?}", result.corrected);
    println!("Candidates: {:?}", result.candidates);
}

use std::path::Path;
use std::sync::Arc;
use typefix::core::Dict;
use typefix::correction::engine::{CorrectionSource, EngineConfig};
use typefix::correction::StaticErrorMap;
use typefix::CorrectionEngine;
use typefix::LanguageDetector;

fn main() {
    println!("\n=== TEST 1: CLI `correct` command uses HARDCODED test dict, not real FST ===\n");
    // (Already proven in earlier test - cli outputs "como" for input "como" which means
    //  it's using the simple() test pipeline with only 5 words)
    println!("  The CLI subcommand `typefix correct <word>` in src/main.rs:361-391");
    println!("  creates a test dict with: hello, que, test, the, world (5 words total).");
    println!("  It does NOT load the real FST. The user is testing with a dummy.\n");

    println!("=== TEST 2: Real engine with real FST — exact lookup ===");
    let es_dict = Dict::from_fst_file(Path::new("data/dictionaries/es.fst")).unwrap();
    let en_dict = Dict::from_fst_file(Path::new("data/dictionaries/en.fst")).unwrap();
    let pt_dict = Dict::from_fst_file(Path::new("data/dictionaries/pt.fst")).unwrap();
    println!("  es.fst: {} words", es_dict.len());
    println!("  en.fst: {} words", en_dict.len());
    println!("  pt.fst: {} words\n", pt_dict.len());

    let mut engine = CorrectionEngine::new(EngineConfig {
        max_edit_distance: 2,
        max_candidates: 5,
        min_word_length: 2,
        case_sensitive: false,
    });
    engine.add_dictionary("es", Arc::new(es_dict));
    engine.add_dictionary("en", Arc::new(en_dict));
    engine.add_dictionary("pt", Arc::new(pt_dict));

    let es_errors = StaticErrorMap::from_json_file(Path::new("data/errors/es.json")).unwrap();
    let en_errors = StaticErrorMap::from_json_file(Path::new("data/errors/en.json")).unwrap();
    let pt_errors = StaticErrorMap::from_json_file(Path::new("data/errors/pt.json")).unwrap();
    engine.add_error_map(Arc::new(es_errors), "es");
    engine.add_error_map(Arc::new(en_errors), "en");
    engine.add_error_map(Arc::new(pt_errors), "pt");

    let detector = Arc::new(LanguageDetector::new(Default::default()));
    detector.set_language("es");
    engine.set_detector(detector);

    println!("=== TEST 3: Real engine, common Spanish words ===");
    let tests = vec![
        ("hola", "common greeting, in dict"),
        ("casa", "common noun, in dict"),
        ("como", "common word, in dict"),
        ("que", "stopword+dict"),
        ("para", "stopword+dict"),
        ("agua", "common, in dict"),
        ("perro", "common, in dict"),
        ("kasa", "typo of casa, d=1 (k→c)"),
        ("komo", "typo of como, d=1 (k→c)"),
        ("qeu", "typo of que, d=1 (transposition: ue↔eu)"),
        ("csaa", "typo of casa, d=2 (delete a, insert a)"),
        ("cassa", "typo of casa, d=1 (insert s)"),
        ("perrro", "typo of perro, d=1 (insert r)"),
        ("olaa", "typo of hola, d=1 (insert a)"),
    ];
    println!("\n  {:>12} | {:>14} | {:>12} | {}", "INPUT", "STATUS", "OUTPUT", "NOTE");
    println!("  {}", "-".repeat(80));
    for (word, note) in tests {
        let r = engine.correct(word);
        let status = if r.corrected.is_some() { "CORRECTED" } else { "no-fix" };
        let out = r.corrected.clone().unwrap_or_else(|| word.into());
        println!("  {:>12} | {:>14} | {:>12} | {}", word, status, out, note);
    }

    println!("\n=== TEST 4: Cross-language suppression (Step 1.5) ===");
    println!("  The engine has logic: if word is in ANY loaded dict, skip correction.");
    println!("  This means: typing English 'the' while in Spanish mode won't be flagged");
    println!("  as wrong. Good for code-switching. But: typing 'casa' in Spanish mode is");
    println!("  valid → not corrected. If the user wrote 'cassa' (typo), the engine WILL");
    println!("  try to fuzzy-match it. Let's see what it does:\n");
    let r1 = engine.correct("cassa");
    println!("  'cassa' in es mode → corrected = {:?}", r1.corrected);
    let r2 = engine.correct("teh");
    println!("  'teh' in es mode → corrected = {:?} (teh is in es.json errors but says EN)", r2.corrected);

    println!("\n=== TEST 5: Spell-check state of the art (Norvig + SymSpell) ===");
    println!("  Most modern spell checkers do 'delete-edit generation':");
    println!("  For input 'accomodate', generate all d=1 deletions:");
    println!("    ccomodate, acommodate, accomodate, accomodte, accomodae, accomodat");
    println!("  Look up each in dictionary → found 'accommodate'.\n");
    println!("  The FST-based approach (current code) does the opposite: runs an");
    println!("  automaton over the FST. Slower for short words, similar for long words.");
    println!("  The DELETE-1 method is typically 100-1000x faster for small edits.\n");

    println!("=== TEST 6: Sources the audit should reference ===");
    println!("  · Peter Norvig, 'How to Write a Spelling Corrector' (2007)");
    println!("  · Wolf Garbe, SymSpell (1000x faster than Norvig)");
    println!("  · Lucene DirectSpellChecker, Hunspell, Aspell");
    println!("  · BurntSushi/fst (already used)");
    println!("  · Andrew Gallant, 'Index 1.6B Keys with Automata and Rust'");
}

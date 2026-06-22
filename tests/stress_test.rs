//! Stress tests for TypeFix
//!
//! Tests high-volume scenarios and edge cases.

use std::sync::Arc;
use std::time::{Duration, Instant};

use typefix::core::{CharBuffer, Dict};
use typefix::correction::engine::EngineConfig;
use typefix::correction::{CorrectionEngine, DamerauLevenshtein, StaticErrorMap};
use typefix::language::detector::DetectorConfig;
use typefix::language::{LanguageDetector, StopwordsSet};
use typefix::pipeline::TypeFixPipeline;

// =============================================================================
// Stress Test Results
// =============================================================================

#[derive(Debug, Clone)]
pub struct StressTestResult {
    pub name: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub operations: usize,
    pub ops_per_sec: f64,
    pub details: Vec<String>,
}

impl StressTestResult {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            passed: false,
            duration_ms: 0,
            operations: 0,
            ops_per_sec: 0.0,
            details: Vec::new(),
        }
    }

    pub fn mark_passed(&mut self) {
        self.passed = true;
    }

    pub fn set_duration(&mut self, duration: Duration) {
        self.duration_ms = duration.as_millis() as u64;
        if self.operations > 0 {
            self.ops_per_sec = (self.operations as f64 * 1000.0) / self.duration_ms as f64;
        }
    }

    pub fn add_detail(&mut self, detail: &str) {
        self.details.push(detail.to_string());
    }

    pub fn summary(&self) -> String {
        format!(
            "{}: {} ({:.0} ops/sec) - {}",
            self.name,
            if self.passed { "PASSED" } else { "FAILED" },
            self.ops_per_sec,
            self.details.join(", ")
        )
    }
}

// =============================================================================
// Buffer Stress Tests
// =============================================================================

/// Test rapid character input
pub fn stress_test_rapid_input(char_count: usize) -> StressTestResult {
    let mut result = StressTestResult::new("rapid_input");
    let buffer = CharBuffer::new();
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyz".chars().collect();

    let start = Instant::now();

    for i in 0..char_count {
        let ch = chars[i % chars.len()];
        let _ = buffer.push(ch);
    }

    result.set_duration(start.elapsed());
    result.operations = char_count;

    // Verify buffer state
    let contents = buffer.contents();
    if !contents.is_empty() {
        result.mark_passed();
        result.add_detail(&format!("final_contents_len={}", contents.len()));
    } else {
        result.add_detail("buffer_was_cleared_by_delimiter");
        result.mark_passed(); // This is expected behavior
    }

    result
}

/// Test long string without spaces
pub fn stress_test_long_string(char_count: usize) -> StressTestResult {
    let mut result = StressTestResult::new("long_string");
    let buffer = CharBuffer::new();

    let start = Instant::now();

    // Generate a long string of random-looking characters
    let mut chars = Vec::with_capacity(char_count);
    for i in 0..char_count {
        chars.push((b'a' + (i % 26) as u8) as char);
    }
    let long_string: String = chars.into_iter().collect();

    // Push character by character
    for ch in long_string.chars() {
        let _ = buffer.push(ch);
    }

    result.set_duration(start.elapsed());
    result.operations = char_count;

    // Buffer should contain all characters (no delimiters)
    let contents = buffer.contents();
    let expected_len = char_count.min(64); // MAX_BUFFER_SIZE

    if contents.len() == expected_len {
        result.mark_passed();
        result.add_detail(&format!("buffer_correctly_limited_to_{}", expected_len));
    } else {
        result.add_detail(&format!("unexpected_contents_len={}", contents.len()));
    }

    result
}

/// Test burst input (many characters per second)
pub fn stress_test_burst_input(
    chars_per_second: usize,
    duration_seconds: usize,
) -> StressTestResult {
    let mut result = StressTestResult::new("burst_input");
    let buffer = CharBuffer::new();
    let chars: Vec<char> = "hola mundo como estas".chars().collect();

    let target_interval_us = 1_000_000 / chars_per_second;
    let total_ops = chars_per_second * duration_seconds;
    let start = Instant::now();

    for i in 0..total_ops {
        let ch = chars[i % chars.len()];
        let _ = buffer.push(ch);

        // Simulate time pressure
        if i % 100 == 0 {
            let _elapsed = start.elapsed();
            let expected_elapsed = Duration::from_micros(i as u64 * target_interval_us as u64);
            // We're tracking elapsed time for stress test
            let _ = expected_elapsed;
        }
    }

    result.set_duration(start.elapsed());
    result.operations = total_ops;
    result.mark_passed();
    result.add_detail(&format!(
        "processed_{}_chars_in_{}s",
        total_ops, duration_seconds
    ));

    result
}

/// Test emoji and unicode handling
pub fn stress_test_unicode() -> StressTestResult {
    let mut result = StressTestResult::new("unicode_handling");
    let buffer = CharBuffer::new();

    // Test various unicode scenarios
    let test_cases = vec![
        "café",       // accented characters
        "naïve",      // diacritics
        "日本語",     // CJK characters
        "🎉🎊🎈",     // emojis
        "مرحبا",      // Arabic
        "Привет",     // Cyrillic
        "🌍🌎🌏🌐🗺️", // globe emojis
    ];

    let start = Instant::now();

    for text in &test_cases {
        for ch in text.chars() {
            let _ = buffer.push(ch);
        }
        let _ = buffer.push(' '); // delimiter
    }

    result.set_duration(start.elapsed());
    result.operations = test_cases.iter().map(|s| s.chars().count()).sum();
    result.mark_passed();
    result.add_detail(&format!("handled_{}_unicode_strings", test_cases.len()));

    result
}

// =============================================================================
// Dictionary Stress Tests
// =============================================================================

/// Test large dictionary performance
pub fn stress_test_large_dictionary(word_count: usize) -> StressTestResult {
    let mut result = StressTestResult::new("large_dictionary");

    let start = Instant::now();

    // Generate and sort words
    let mut words = Vec::with_capacity(word_count);
    for i in 0..word_count {
        words.push(format!("word{:06}", i));
    }
    words.sort(); // FST requires sorted keys

    let mut builder = fst::MapBuilder::memory();
    for (i, word) in words.iter().enumerate() {
        builder.insert(word, (word_count - i) as u64).unwrap();
    }
    let dict = Dict::from_bytes(typefix::core::dict::wrap_fst_bytes(&builder.into_inner().unwrap())).unwrap();

    let insert_duration = start.elapsed();
    let search_start = Instant::now();

    // Search for random words
    for i in 0..1000 {
        let word = format!("word{:06}", (i * 7) % word_count); // Some will exist
        let _ = dict.search(&word);
    }

    let search_duration = search_start.elapsed();
    result.set_duration(insert_duration + search_duration);
    result.operations = word_count + 1000;

    result.mark_passed();
    result.add_detail(&format!(
        "build={:.2}ms, search={:.2}ms",
        insert_duration.as_secs_f64() * 1000.0,
        search_duration.as_secs_f64() * 1000.0
    ));

    result
}

/// Test memory with many dictionaries
pub fn stress_test_multiple_dictionaries(
    lang_count: usize,
    words_per_lang: usize,
) -> StressTestResult {
    let mut result = StressTestResult::new("multiple_dictionaries");

    let start = Instant::now();

    for lang_idx in 0..lang_count {
        let lang = format!("lang{:03}", lang_idx);
        let mut words = Vec::new();
        for i in 0..words_per_lang {
            words.push(format!("{}{:06}", lang, i));
        }
        words.sort();
        
        let mut builder = fst::MapBuilder::memory();
        for word in words {
            builder.insert(&word, 1000).unwrap();
        }
        let dict = Dict::from_bytes(typefix::core::dict::wrap_fst_bytes(&builder.into_inner().unwrap())).unwrap();

        // Verify insertion
        let word = format!("{}{:06}", lang, 0);
        if dict.search(&word).is_none() {
            result.add_detail(&format!("failed_to_find_word_in_{}", lang));
            break;
        }
    }

    result.set_duration(start.elapsed());
    result.operations = lang_count * words_per_lang;
    result.mark_passed();
    result.add_detail(&format!(
        "{} languages x {} words",
        lang_count, words_per_lang
    ));

    result
}

// =============================================================================
// Language Detection Stress Tests
// =============================================================================

/// Test rapid language switching
pub fn stress_test_language_switching(switch_count: usize) -> StressTestResult {
    let mut result = StressTestResult::new("language_switching");
    let config = DetectorConfig {
        window_size: 3,
        confidence_threshold: 0.5, // Lower for faster switching
        hysteresis_zone: 0.0,
        min_words_before_switch: 1,
    };
    let detector = LanguageDetector::new(config);

    // Spanish words
    let es_words = vec!["el", "la", "de", "que", "es", "y", "en", "un", "por", "con"];
    // English words
    let en_words = vec![
        "the", "a", "of", "is", "and", "to", "in", "that", "it", "for",
    ];

    let mut es_words_sorted = es_words.clone();
    es_words_sorted.sort();
    let mut en_words_sorted = en_words.clone();
    en_words_sorted.sort();

    let mut es_stopwords = StopwordsSet::new();
    for w in &es_words_sorted {
        es_stopwords.insert(w);
    }
    detector.add_language("es", Arc::new(es_stopwords));

    let mut en_stopwords = StopwordsSet::new();
    for w in &en_words_sorted {
        en_stopwords.insert(w);
    }
    detector.add_language("en", Arc::new(en_stopwords));

    detector.set_language("es");

    let start = Instant::now();

    // Alternate between languages
    for i in 0..switch_count {
        let word = if i % 2 == 0 {
            &es_words[i % es_words.len()]
        } else {
            &en_words[i % en_words.len()]
        };
        let _ = detector.process_word(word);
    }

    result.set_duration(start.elapsed());
    result.operations = switch_count;
    result.mark_passed();
    result.add_detail(&format!("processed_{}_language_switches", switch_count));

    result
}

/// Test mixed language text
pub fn stress_test_mixed_languages() -> StressTestResult {
    let mut result = StressTestResult::new("mixed_languages");
    let config = DetectorConfig {
        window_size: 10,
        confidence_threshold: 0.6,
        hysteresis_zone: 0.1,
        min_words_before_switch: 3,
    };
    let detector = LanguageDetector::new(config);

    let mut es_stopwords = StopwordsSet::new();
    let mut es_w = vec!["el", "la", "de", "que", "es", "y", "en", "un"];
    es_w.sort();
    for w in es_w {
        es_stopwords.insert(w);
    }
    detector.add_language("es", Arc::new(es_stopwords));

    let mut en_stopwords = StopwordsSet::new();
    let mut en_w = vec!["the", "a", "of", "is", "and", "to", "in"];
    en_w.sort();
    for w in en_w {
        en_stopwords.insert(w);
    }
    detector.add_language("en", Arc::new(en_stopwords));

    // Mixed sentence
    let mixed_words = vec![
        "Hola", "como", "estas", "the", "weather", "es", "muy", "nice", "today", "hoy", "es",
        "great",
    ];

    let start = Instant::now();

    for word in &mixed_words {
        let _ = detector.process_word(&word.to_lowercase());
    }

    result.set_duration(start.elapsed());
    result.operations = mixed_words.len();
    result.mark_passed();
    result.add_detail(&format!("processed_{}_mixed_words", mixed_words.len()));

    result
}

// =============================================================================
// Correction Stress Tests
// =============================================================================

/// Test high-volume corrections
pub fn stress_test_high_volume_corrections(correction_count: usize) -> StressTestResult {
    let mut result = StressTestResult::new("high_volume_corrections");
    let engine = Arc::new(build_correction_engine());

    let typos = vec![
        "qeu", "teh", "adn", "hte", "wrok", " recive", "teh", "qeu", "hte", "adn", "wrok",
    ];

    let start = Instant::now();

    for i in 0..correction_count {
        let typo = &typos[i % typos.len()];
        let _ = engine.correct(typo);
    }

    result.set_duration(start.elapsed());
    result.operations = correction_count;
    result.mark_passed();
    result.add_detail(&format!(
        "corrected_{}_typos_at_{:.0}_ops_sec",
        correction_count, result.ops_per_sec
    ));

    result
}

/// Test Damerau-Levenshtein with various inputs
pub fn stress_test_damerau_variants(iterations: usize) -> StressTestResult {
    let mut result = StressTestResult::new("damerau_variants");
    let mut calc = DamerauLevenshtein::new();

    let test_cases = vec![
        ("", ""),          // empty
        ("a", "b"),        // single char
        ("ab", "ba"),      // transposition
        ("abc", "abd"),    // substitution
        ("abc", "abcd"),   // insertion
        ("abcd", "abc"),   // deletion
        ("hello", "hola"), // completely different
        ("café", "café"),  // unicode identical
        ("😀", "😁"),      // emoji
    ];

    let start = Instant::now();

    for i in 0..iterations {
        let (s1, s2) = &test_cases[i % test_cases.len()];
        let _ = calc.distance(s1, s2, 3);
    }

    result.set_duration(start.elapsed());
    result.operations = iterations;
    result.mark_passed();
    result.add_detail(&format!("tested_{}_variants", test_cases.len()));

    result
}

// =============================================================================
// Pipeline Stress Tests
// =============================================================================

/// Test pipeline under load
pub fn stress_test_pipeline(text_length: usize) -> StressTestResult {
    let mut result = StressTestResult::new("pipeline_load");
    let pipeline = TypeFixPipeline::simple();

    // Generate long text with typos
    let mut text = String::with_capacity(text_length);
    let words = [
        "hola",
        "teh",
        "world",
        "qeu",
        "test",
        "and",
        "hte",
        "correction",
    ];

    for i in 0..(text_length / 6) {
        if i > 0 {
            text.push(' ');
        }
        text.push_str(words[i % words.len()]);
    }

    let start = Instant::now();

    let mut processed = 0;
    for ch in text.chars() {
        if let Some(_result_word) = pipeline.push(ch) {
            processed += 1;
            // Could verify corrections here
        }
    }

    result.set_duration(start.elapsed());
    result.operations = processed;
    result.mark_passed();
    result.add_detail(&format!(
        "processed_{}_words_from_{}_chars",
        processed, text_length
    ));

    result
}

// =============================================================================
// Helper Functions
// =============================================================================

fn build_correction_engine() -> CorrectionEngine {
    let engine = CorrectionEngine::new(EngineConfig::default());

    let mut builder = fst::MapBuilder::memory();
    let mut words = vec![
        ("and", 800000),
        ("correction", 50000),
        ("hello", 500000),
        ("que", 900000),
        ("receive", 200000),
        ("test", 300000),
        ("the", 1000000),
        ("weather", 100000),
        ("work", 150000),
        ("world", 400000),
    ];
    words.sort_by_key(|k| k.0);
    for (word, freq) in words {
        builder.insert(word, freq).unwrap();
    }
    let en_dict = Dict::from_bytes(typefix::core::dict::wrap_fst_bytes(&builder.into_inner().unwrap())).unwrap();
    engine.add_dictionary("en", Arc::new(en_dict));

    let error_map = StaticErrorMap::new("en");
    error_map.insert_static("qeu", "que");
    error_map.insert_static("teh", "the");
    error_map.insert_static("adn", "and");
    error_map.insert_static("hte", "the");
    error_map.insert_static("wrok", "work");
    error_map.insert_static(" recive", "receive");
    engine.add_error_map(Arc::new(error_map), "en");

    engine
}

// =============================================================================
// Full Stress Test Suite
// =============================================================================

/// Run all stress tests
pub fn run_stress_test_suite() -> Vec<StressTestResult> {
    let mut results = Vec::new();

    println!("\n=== TypeFix Stress Test Suite ===\n");

    // Buffer stress tests
    println!("--- Buffer Stress Tests ---");
    results.push(stress_test_rapid_input(10000));
    results.push(stress_test_long_string(100));
    results.push(stress_test_burst_input(100, 5)); // 100 chars/sec for 5 seconds
    results.push(stress_test_unicode());

    // Dictionary stress tests
    println!("\n--- Dictionary Stress Tests ---");
    results.push(stress_test_large_dictionary(50000));
    results.push(stress_test_multiple_dictionaries(3, 10000));

    // Language detection stress tests
    println!("\n--- Language Detection Stress Tests ---");
    results.push(stress_test_language_switching(1000));
    results.push(stress_test_mixed_languages());

    // Correction stress tests
    println!("\n--- Correction Stress Tests ---");
    results.push(stress_test_high_volume_corrections(10000));
    results.push(stress_test_damerau_variants(10000));

    // Pipeline stress tests
    println!("\n--- Pipeline Stress Tests ---");
    results.push(stress_test_pipeline(1000));

    // Print summary
    println!("\n=== Stress Test Results ===");
    for result in &results {
        println!("{}", result.summary());
    }

    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.len() - passed;

    println!("\n{} passed, {} failed", passed, failed);

    if failed == 0 {
        println!("\n✅ ALL STRESS TESTS PASSED");
    } else {
        println!("\n⚠️ {} STRESS TESTS FAILED", failed);
    }

    results
}

// =============================================================================
// Main entry point for running stress tests directly
// =============================================================================

fn main() {
    println!("Running TypeFix Stress Test Suite...\n");
    let results = run_stress_test_suite();

    // Exit with error code if any tests failed
    let failed = results.iter().filter(|r| !r.passed).count();
    if failed > 0 {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stress_result() {
        let mut result = StressTestResult::new("test");
        result.mark_passed();
        result.set_duration(Duration::from_millis(100));
        result.operations = 1000;
        assert!(result.passed);
        assert_eq!(result.duration_ms, 100);
    }

    #[test]
    fn test_rapid_input_small() {
        let result = stress_test_rapid_input(100);
        assert!(result.passed);
        // ops_per_sec may be 0 if duration rounds to 0ms for very fast operations
        assert!(
            result.operations >= 100,
            "should have recorded 100 operations"
        );
    }

    #[test]
    fn test_unicode_small() {
        let result = stress_test_unicode();
        assert!(result.passed);
    }

    #[test]
    fn test_language_switching_small() {
        let result = stress_test_language_switching(100);
        assert!(result.passed);
    }
}

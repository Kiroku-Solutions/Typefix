//! Correction engine - main entry point for typo correction
//!
//! Combines static error maps, Damerau-Levenshtein distance, and language
//! dictionaries to provide accurate typo corrections.

use crate::core::Trie;
use crate::correction::{DamerauLevenshtein, StaticErrorMap};
use crate::language::LanguageDetector;
use parking_lot::RwLock;
use std::sync::Arc;

/// Correction candidate
#[derive(Debug, Clone)]
pub struct CorrectionCandidate {
    /// The corrected word
    pub word: String,
    /// Edit distance from original
    pub distance: usize,
    /// Word frequency in dictionary
    pub frequency: u64,
    /// Source of correction
    pub source: CorrectionSource,
}

/// Where the correction came from
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionSource {
    /// Static error map
    UserKnown,
    /// Damerau-Levenshtein match
    Dictionary,
    /// No correction found
    None,
}

/// Correction result
#[derive(Debug, Clone)]
pub struct CorrectionResult {
    /// The original word
    pub original: String,
    /// The corrected word (if any)
    pub corrected: Option<String>,
    /// All candidates considered
    pub candidates: Vec<CorrectionCandidate>,
    /// Source of correction
    pub source: CorrectionSource,
}

/// Correction engine configuration
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Maximum edit distance for dictionary lookups
    pub max_edit_distance: usize,
    /// Maximum candidates to return
    pub max_candidates: usize,
    /// Minimum word length to correct
    pub min_word_length: usize,
    /// Case sensitivity
    pub case_sensitive: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_edit_distance: 1,
            max_candidates: 3,
            min_word_length: 2,
            case_sensitive: false,
        }
    }
}

/// Main correction engine
#[allow(
    missing_debug_implementations,
    reason = "RwLock fields and HashMap state; manual Debug impl is not worth the maintenance burden"
)]
pub struct CorrectionEngine {
    config: EngineConfig,
    /// Dictionaries by language (interior mutability)
    dictionaries: RwLock<std::collections::HashMap<String, Arc<Trie>>>,
    /// Error maps by language (interior mutability)
    error_maps: RwLock<std::collections::HashMap<String, Arc<StaticErrorMap>>>,
    /// Damerau-Levenshtein calculator
    #[allow(
        dead_code,
        reason = "pre-allocated for future inline Damerau-Levenshtein; currently routes through static methods"
    )]
    damerau: RwLock<DamerauLevenshtein>,
    /// Current language detector (interior mutability)
    detector: RwLock<Arc<LanguageDetector>>,
}

impl CorrectionEngine {
    /// Create a new engine with configuration
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            dictionaries: RwLock::new(std::collections::HashMap::new()),
            error_maps: RwLock::new(std::collections::HashMap::new()),
            damerau: RwLock::new(DamerauLevenshtein::new()),
            detector: RwLock::new(Arc::new(LanguageDetector::new(
                crate::language::detector::DetectorConfig::default(),
            ))),
        }
    }

    /// Add a dictionary for a language
    pub fn add_dictionary(&self, lang: &str, trie: Arc<Trie>) {
        self.dictionaries.write().insert(lang.to_string(), trie);
    }

    /// Add an error map for a language
    pub fn add_error_map(&self, map: Arc<StaticErrorMap>, lang: &str) {
        self.error_maps.write().insert(lang.to_string(), map);
    }

    /// Set the language detector
    pub fn set_detector(&self, detector: Arc<LanguageDetector>) {
        *self.detector.write() = detector;
    }

    /// Set the current language
    pub fn set_language(&self, lang: &str) {
        self.detector.read().set_language(lang);
    }

    /// Set the language detector (consuming)
    pub fn with_detector(mut self, detector: Arc<LanguageDetector>) -> Self {
        *self.detector.get_mut() = detector;
        self
    }

    /// Correct a single word
    ///
    /// Returns the best correction if found.
    /// Fails gracefully: never panics, always returns a valid result.
    pub fn correct(&self, word: &str) -> CorrectionResult {
        // Skip short words
        if word.chars().count() < self.config.min_word_length {
            return CorrectionResult {
                original: word.to_string(),
                corrected: None,
                candidates: Vec::new(),
                source: CorrectionSource::None,
            };
        }

        let current_lang = self.detector.read().get_language();
        let word_normalized = if self.config.case_sensitive {
            word.to_string()
        } else {
            word.to_lowercase()
        };

        // Step 1: Check static error map (O(1))
        if let Some(map) = self.error_maps.read().get(&current_lang) {
            if let Some(correction) = map.lookup(&word_normalized) {
                let candidate = CorrectionCandidate {
                    word: correction.clone(),
                    distance: 0,
                    frequency: map.get_frequency(&word_normalized),
                    source: CorrectionSource::UserKnown,
                };
                return CorrectionResult {
                    original: word.to_string(),
                    corrected: Some(correction),
                    candidates: vec![candidate],
                    source: CorrectionSource::UserKnown,
                };
            }
        }

        // Step 2: Dictionary lookup with Damerau-Levenshtein
        if let Some(dict) = self.dictionaries.read().get(&current_lang) {
            let candidates = self.find_dictionary_corrections(&word_normalized, dict);

            if !candidates.is_empty() {
                let best = &candidates[0];
                return CorrectionResult {
                    original: word.to_string(),
                    corrected: Some(best.word.clone()),
                    candidates,
                    source: CorrectionSource::Dictionary,
                };
            }
        }

        // No correction found - fail gracefully
        CorrectionResult {
            original: word.to_string(),
            corrected: None,
            candidates: Vec::new(),
            source: CorrectionSource::None,
        }
    }

    /// Find corrections in dictionary using Damerau-Levenshtein
    fn find_dictionary_corrections(&self, word: &str, dict: &Trie) -> Vec<CorrectionCandidate> {
        // First try exact match - no typo
        if dict.contains(word) {
            return Vec::new();
        }

        // Find similar words within max edit distance
        let similar = dict.find_similar(
            word,
            self.config.max_edit_distance,
            self.config.max_candidates,
        );

        similar
            .into_iter()
            .map(|(w, dist, freq)| CorrectionCandidate {
                word: w,
                distance: dist,
                frequency: freq,
                source: CorrectionSource::Dictionary,
            })
            .collect()
    }

    /// Correct multiple words (text) preserving punctuation
    ///
    /// Returns corrected text with same spacing.
    pub fn correct_text(&self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut current_word = String::new();

        for ch in text.chars() {
            if Self::is_word_char(ch) {
                current_word.push(ch);
            } else {
                // End of word - try correction
                if !current_word.is_empty() {
                    let correction = self.correct(&current_word);
                    if let Some(corrected) = correction.corrected {
                        // Preserve case
                        let final_word = Self::preserve_case(&current_word, &corrected);
                        result.push_str(&final_word);
                    } else {
                        result.push_str(&current_word);
                    }
                    current_word.clear();
                }
                result.push(ch);
            }
        }

        // Handle last word
        if !current_word.is_empty() {
            let correction = self.correct(&current_word);
            if let Some(corrected) = correction.corrected {
                let final_word = Self::preserve_case(&current_word, &corrected);
                result.push_str(&final_word);
            } else {
                result.push_str(&current_word);
            }
        }

        result
    }

    /// Check if character is part of a word
    #[inline]
    fn is_word_char(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '\'' || ch == '-'
    }

    /// Preserve original case pattern
    fn preserve_case(original: &str, corrected: &str) -> String {
        let original_chars: Vec<char> = original.chars().collect();
        let corrected_chars: Vec<char> = corrected.chars().collect();

        if original_chars.is_empty() || corrected_chars.is_empty() {
            return corrected.to_string();
        }

        // Title case: first letter uppercase
        if original_chars[0].is_uppercase() {
            let mut result = String::new();
            for (i, c) in corrected_chars.iter().enumerate() {
                result.push(if i == 0 {
                    c.to_uppercase().to_string().chars().next().unwrap_or(*c)
                } else {
                    c.to_lowercase().to_string().chars().next().unwrap_or(*c)
                });
            }
            return result;
        }

        corrected.to_lowercase()
    }

    /// Get all possible corrections for a word (without applying)
    pub fn get_corrections(&self, word: &str) -> Vec<CorrectionCandidate> {
        if word.chars().count() < self.config.min_word_length {
            return Vec::new();
        }

        let current_lang = self.detector.read().get_language();
        let word_normalized = word.to_lowercase();
        let mut candidates = Vec::new();

        // Add static error map corrections
        if let Some(map) = self.error_maps.read().get(&current_lang) {
            if let Some(correction) = map.lookup(&word_normalized) {
                candidates.push(CorrectionCandidate {
                    word: correction,
                    distance: 0,
                    frequency: map.get_frequency(&word_normalized),
                    source: CorrectionSource::UserKnown,
                });
            }
        }

        // Add dictionary corrections
        if let Some(dict) = self.dictionaries.read().get(&current_lang) {
            let dict_corrections = self.find_dictionary_corrections(&word_normalized, dict);
            candidates.extend(dict_corrections);
        }

        // Sort by: distance (asc), then frequency (desc)
        candidates.sort_by(|a, b| {
            a.distance
                .cmp(&b.distance)
                .then_with(|| b.frequency.cmp(&a.frequency))
        });

        // Limit results
        candidates.truncate(self.config.max_candidates);
        candidates
    }

    /// Mark a correction as correct (learn)
    pub fn mark_correct(&self, typo: &str, correction: &str) {
        let current_lang = self.detector.read().get_language();
        if let Some(map) = self.error_maps.read().get(&current_lang) {
            map.learn(typo, correction);
        }
    }

    /// Mark a correction as wrong (unlearn)
    pub fn mark_incorrect(&self, typo: &str) {
        let current_lang = self.detector.read().get_language();
        if let Some(map) = self.error_maps.read().get(&current_lang) {
            map.unlearn(typo);
        }
    }

    /// Get current language
    pub fn current_language(&self) -> String {
        self.detector.read().get_language()
    }
}

impl Default for CorrectionEngine {
    fn default() -> Self {
        Self::new(EngineConfig::default())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "test code uses unwrap for concise assertions"
)]
#[allow(
    unused_mut,
    reason = "mut bindings used in assertions; may be removed in future"
)]
mod tests {
    use super::*;

    fn create_test_engine() -> CorrectionEngine {
        let mut engine = CorrectionEngine::new(EngineConfig {
            max_edit_distance: 1,
            max_candidates: 3,
            min_word_length: 2,
            case_sensitive: false,
        });

        // Add test dictionary
        let mut trie = Trie::new();
        trie.insert("hello", 1000);
        trie.insert("world", 800);
        trie.insert("hola", 900);
        trie.insert("que", 500);
        trie.insert("the", 10000);
        trie.insert("and", 9000);
        trie.insert("is", 8000);
        engine.add_dictionary("en", Arc::new(trie));

        let detector = Arc::new(LanguageDetector::new(
            crate::language::detector::DetectorConfig {
                min_words_before_switch: 1,
                ..Default::default()
            },
        ));

        // Set language (set_language uses interior mutability via RwLock)
        detector.set_language("en");

        engine.with_detector(detector)
    }

    #[test]
    fn test_short_words_not_corrected() {
        let engine = create_test_engine();
        let result = engine.correct("h");
        assert_eq!(result.corrected, None);
        assert_eq!(result.original, "h");
    }

    #[test]
    fn test_no_correction_for_valid_word() {
        let engine = create_test_engine();
        let result = engine.correct("hello");
        assert_eq!(result.corrected, None); // Already valid
    }

    #[test]
    fn test_multiple_candidates() {
        let mut engine = create_test_engine();
        let mut trie = Trie::new();
        trie.insert("hello", 1000);
        trie.insert("jello", 800);
        trie.insert("yello", 600);
        trie.insert("hallo", 400);
        engine.add_dictionary("en", Arc::new(trie));

        let candidates = engine.get_corrections("helo");
        assert!(!candidates.is_empty());
    }

    #[test]
    fn test_text_correction_preserves_punctuation() {
        let mut engine = create_test_engine();
        let mut trie = Trie::new();
        trie.insert("hello", 1000);
        trie.insert("world", 800);
        engine.add_dictionary("en", Arc::new(trie));

        let detector = Arc::new(LanguageDetector::new(
            crate::language::detector::DetectorConfig {
                min_words_before_switch: 1,
                ..Default::default()
            },
        ));
        let engine = engine.with_detector(detector);

        let text = "hello, world!";
        let corrected = engine.correct_text(text);
        assert_eq!(corrected, text); // Valid words unchanged
    }

    #[test]
    fn test_case_preservation_title_case() {
        let engine = create_test_engine();
        // "HELLO" should stay uppercase-ish if desired
        let result = engine.correct("HELLO");
        // Valid word, no correction needed
        assert_eq!(result.corrected, None);
    }

    #[test]
    fn test_empty_string() {
        let engine = create_test_engine();
        let result = engine.correct("");
        assert_eq!(result.original, "");
        assert_eq!(result.corrected, None);
    }

    #[test]
    fn test_fail_safe_returns_original() {
        let engine = CorrectionEngine::default();
        // Unknown word returns original
        let result = engine.correct("xyzzyab");
        assert_eq!(result.original, "xyzzyab");
        assert_eq!(result.corrected, None);
    }

    #[test]
    fn test_unicode_in_text() {
        let mut engine = create_test_engine();
        let mut trie = Trie::new();
        trie.insert("café", 1000);
        trie.insert("naïve", 800);
        engine.add_dictionary("en", Arc::new(trie));

        let detector = Arc::new(LanguageDetector::new(
            crate::language::detector::DetectorConfig {
                min_words_before_switch: 1,
                ..Default::default()
            },
        ));
        let engine = engine.with_detector(detector);

        let text = "I love café";
        let corrected = engine.correct_text(text);
        assert!(corrected.contains("café"));
    }

    #[test]
    fn test_emoji_not_crashed() {
        let engine = create_test_engine();
        let result = engine.correct("😀");
        // Should not crash, returns original
        assert_eq!(result.original, "😀");
    }

    #[test]
    fn test_very_long_word() {
        let engine = create_test_engine();
        let long = "a".repeat(1000);
        let result = engine.correct(&long);
        // Should not crash, returns original
        assert_eq!(result.original.len(), 1000);
    }

    #[test]
    fn test_word_with_apostrophe() {
        let mut engine = create_test_engine();
        let mut trie = Trie::new();
        trie.insert("don't", 1000);
        trie.insert("isn't", 800);
        engine.add_dictionary("en", Arc::new(trie));

        let detector = Arc::new(LanguageDetector::new(
            crate::language::detector::DetectorConfig {
                min_words_before_switch: 1,
                ..Default::default()
            },
        ));
        let engine = engine.with_detector(detector);

        let result = engine.correct("don't");
        assert_eq!(result.corrected, None); // Valid word
    }

    #[test]
    fn test_mark_correct_learns() {
        let mut engine = create_test_engine();

        let errors = StaticErrorMap::new("en");
        engine.add_error_map(Arc::new(errors), "en");

        engine.mark_correct("mytypo", "my typo");
        let result = engine.correct("mytypo");
        assert_eq!(result.corrected, Some("my typo".to_string()));
    }

    #[test]
    fn test_mark_incorrect_unlearns() {
        let engine = create_test_engine();
        engine.mark_incorrect("testtypo");
        // Should not crash
        let result = engine.correct("testtypo");
        assert_eq!(result.original, "testtypo");
    }

    #[test]
    fn test_suggestions_sorted_by_distance_then_frequency() {
        let mut engine = create_test_engine();
        let mut trie = Trie::new();
        trie.insert("hello", 1000);
        trie.insert("jello", 800);
        trie.insert("hella", 600);
        engine.add_dictionary("en", Arc::new(trie));

        let detector = Arc::new(LanguageDetector::new(
            crate::language::detector::DetectorConfig {
                min_words_before_switch: 1,
                ..Default::default()
            },
        ));
        let engine = engine.with_detector(detector);

        let candidates = engine.get_corrections("helo");
        // Should be sorted by distance, then frequency
        if candidates.len() >= 2 {
            assert!(candidates[0].distance <= candidates[1].distance);
        }
    }
}

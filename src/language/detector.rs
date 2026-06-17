//! Language detection using Bayesian inference
//!
//! Tracks a rolling window of words and calculates the probability
//! that the current text is in each supported language.

use crate::core::Trie;
use anyhow::Result;
use parking_lot::RwLock;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

/// Detection result
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Detected language (ISO 639-1)
    pub language: String,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Scores for all languages
    pub all_scores: HashMap<String, f64>,
}

/// Bayesian language detector
///
/// Uses a rolling window of words to calculate language probability.
/// Requires stopwords trie to be loaded for each language.
#[allow(
    missing_debug_implementations,
    reason = "RwLock fields and internal state; manual Debug impl is not worth the maintenance burden"
)]
pub struct LanguageDetector {
    config: DetectorConfig,
    /// Rolling window of recent words (interior mutability)
    word_window: RwLock<Vec<String>>,
    /// Pre-calculated language probabilities (from corpus) (interior mutability)
    language_priors: RwLock<HashMap<String, f64>>,
    /// Stopwords tries by language (interior mutability)
    stopwords_tries: RwLock<HashMap<String, Arc<StopwordsTrie>>>,
    /// Current detected language
    current_language: RwLock<String>,
}

/// Configuration controlling Bayesian language detection behavior
#[derive(Debug, Clone)]
pub struct DetectorConfig {
    /// Size of the rolling window for word collection
    pub window_size: usize,
    /// Confidence threshold for language detection (0.0-1.0)
    pub confidence_threshold: f64,
    /// Hysteresis zone to prevent rapid language switching
    pub hysteresis_zone: f64,
    /// Minimum words before considering a language switch
    pub min_words_before_switch: usize,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            window_size: 5,
            confidence_threshold: 0.85,
            hysteresis_zone: 0.10,
            min_words_before_switch: 5,
        }
    }
}

impl LanguageDetector {
    /// Create a new detector with configuration
    pub fn new(config: DetectorConfig) -> Self {
        Self {
            config: config.clone(),
            word_window: RwLock::new(Vec::with_capacity(config.window_size)),
            language_priors: RwLock::new(HashMap::new()),
            stopwords_tries: RwLock::new(HashMap::new()),
            current_language: RwLock::new(String::new()),
        }
    }

    /// Add a stopwords trie for a language
    pub fn add_language(&self, lang: &str, stopwords: Arc<StopwordsTrie>) {
        {
            let mut tries = self.stopwords_tries.write();
            tries.insert(lang.to_string(), stopwords);
        }
        // Default prior: uniform distribution
        let count = self.stopwords_tries.read().len();
        if count > 0 {
            let mut priors = self.language_priors.write();
            for l in self.stopwords_tries.read().keys() {
                priors.insert(l.clone(), 1.0 / count as f64);
            }
        }
    }

    /// Set the current language (initial state)
    pub fn set_language(&self, lang: &str) {
        let mut current = self.current_language.write();
        *current = lang.to_string();
    }

    /// Get the current detected language
    pub fn get_language(&self) -> String {
        self.current_language.read().clone()
    }

    /// Add a word to the window and recalculate
    ///
    /// Returns detection result if language changed
    pub fn process_word(&self, word: &str) -> Option<DetectionResult> {
        let word_lower = word.to_lowercase();

        // Add to window
        {
            let mut window = self.word_window.write();
            window.push(word_lower.clone());
            if window.len() > self.config.window_size {
                window.remove(0);
            }
        }

        // Need at least min_words_before_switch words
        if self.word_window.read().len() < self.config.min_words_before_switch {
            return None;
        }

        // Calculate scores for each language
        let scores = self.calculate_scores();

        // Find best language
        let best_lang: String;
        let best_score: f64;
        if let Some((lang, score)) = scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            best_lang = lang.clone();
            best_score = *score;
        } else {
            return None;
        }

        // Check if we should switch
        let should_switch = self.should_switch_language(&best_lang, best_score);

        if should_switch {
            let mut current = self.current_language.write();
            if *current != best_lang {
                *current = best_lang.clone();
                return Some(DetectionResult {
                    language: best_lang.clone(),
                    confidence: best_score,
                    all_scores: scores,
                });
            }
        }

        None
    }

    /// Calculate probability scores for each language
    fn calculate_scores(&self) -> HashMap<String, f64> {
        let tries = self.stopwords_tries.read();
        let window = self.word_window.read();

        let mut scores: HashMap<String, f64> = tries.keys().map(|l| (l.clone(), 0.0f64)).collect();

        for word in window.iter() {
            for (lang, trie) in tries.iter() {
                if trie.is_stopword(word) {
                    if let Some(score) = scores.get_mut(lang) {
                        *score += 1.0;
                    }
                }
            }
        }

        // Normalize to probability
        let total: f64 = scores.values().sum();
        if total > 0.0 {
            for score in scores.values_mut() {
                *score /= total;
            }
        }

        scores
    }

    /// Determine if we should switch language
    fn should_switch_language(&self, _new_lang: &str, new_score: f64) -> bool {
        let current_lang = self.current_language.read();

        // Above threshold: confident enough to switch
        if new_score >= self.config.confidence_threshold {
            return true;
        }

        // Within hysteresis zone: only switch if significantly better
        let hysteresis_threshold = self.config.confidence_threshold - self.config.hysteresis_zone;
        if new_score >= hysteresis_threshold {
            // Check if new language is significantly better than current
            let current_score = self.calculate_current_score(&current_lang);
            // Switch if new score is at least 20% better
            return new_score > current_score * 1.2;
        }

        false
    }

    /// Calculate score for a specific language
    fn calculate_current_score(&self, lang: &str) -> f64 {
        let window = self.word_window.read();
        let tries = self.stopwords_tries.read();

        tries.get(lang).map_or(0.0, |trie| {
            let count = window.iter().filter(|w| trie.is_stopword(w)).count() as f64;
            count / window.len().max(1) as f64
        })
    }

    /// Reset the word window
    pub fn reset(&self) {
        self.word_window.write().clear();
    }

    /// Get window contents
    pub fn get_window(&self) -> Vec<String> {
        self.word_window.read().clone()
    }

    /// Get the number of words in the window
    pub fn window_len(&self) -> usize {
        self.word_window.read().len()
    }
}

/// Stopwords Trie for efficient language detection
///
/// Stores stopwords (common words) for quick language fingerprinting.
#[derive(Debug, Clone, Default)]
pub struct StopwordsTrie {
    trie: Trie,
    stopword_count: usize,
}

impl StopwordsTrie {
    /// Create a new empty StopwordsTrie
    pub fn new() -> Self {
        Self {
            trie: Trie::new(),
            stopword_count: 0,
        }
    }

    /// Insert a stopword
    pub fn insert(&mut self, word: &str) {
        if !self.trie.contains(word) {
            self.stopword_count += 1;
        }
        self.trie.insert_word(word);
    }

    /// Check if a word is a stopword
    pub fn is_stopword(&self, word: &str) -> bool {
        self.trie.contains(&word.to_lowercase())
    }

    /// Get the number of stopwords
    pub fn len(&self) -> usize {
        self.stopword_count
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.stopword_count == 0
    }

    /// Load from JSON file
    pub fn from_json_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_json(&content)
    }

    /// Load from JSON string
    pub fn from_json(json: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct StopwordsFile {
            stopwords: Vec<String>,
        }

        let file: StopwordsFile = serde_json::from_str(json)?;
        let mut trie = StopwordsTrie::new();

        for word in file.stopwords {
            trie.insert(&word);
        }

        Ok(trie)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "test code uses unwrap for concise assertions"
)]
#[allow(
    unused_mut,
    reason = "mut bindings reserved for future assertions and mutation"
)]
#[allow(
    unused_variables,
    reason = "result variables used implicitly by side-effecting test code"
)]
mod tests {
    use super::*;

    #[test]
    fn test_stopwords_trie() {
        let mut trie = StopwordsTrie::new();
        trie.insert("el");
        trie.insert("la");
        trie.insert("de");
        trie.insert("que");

        assert!(trie.is_stopword("el"));
        assert!(trie.is_stopword("EL")); // Case insensitive
        assert!(!trie.is_stopword("hola"));
        assert_eq!(trie.len(), 4);
    }

    #[test]
    fn test_language_detector_basic() {
        let mut detector = LanguageDetector::new(DetectorConfig {
            window_size: 5,
            confidence_threshold: 0.6, // Lower threshold
            hysteresis_zone: 0.0,
            min_words_before_switch: 2, // Only need 2 words
        });

        let mut es_stopwords = StopwordsTrie::new();
        es_stopwords.insert("el");
        es_stopwords.insert("la");

        let mut en_stopwords = StopwordsTrie::new();

        detector.add_language("es", Arc::new(es_stopwords));
        detector.add_language("en", Arc::new(en_stopwords));
        detector.set_language("en");

        // Two Spanish stopwords should trigger switch
        let result1 = detector.process_word("el");
        let result2 = detector.process_word("la");

        // After 2 words, should detect Spanish
        assert!(result2.is_some());
        assert_eq!(result2.unwrap().language, "es");
    }

    #[test]
    fn test_no_switch_on_insufficient_words() {
        let mut detector = LanguageDetector::new(DetectorConfig {
            window_size: 5,
            confidence_threshold: 0.85,
            hysteresis_zone: 0.10,
            min_words_before_switch: 3,
        });

        let mut es_stopwords = StopwordsTrie::new();
        es_stopwords.insert("el");

        detector.add_language("es", Arc::new(es_stopwords));
        detector.set_language("en");

        // Only one word - should not switch
        let result = detector.process_word("el");
        assert!(result.is_none());
        assert_eq!(detector.get_language(), "en");
    }

    #[test]
    fn test_hysteresis() {
        let mut detector = LanguageDetector::new(DetectorConfig {
            window_size: 5,
            confidence_threshold: 0.85,
            hysteresis_zone: 0.30, // Large hysteresis zone
            min_words_before_switch: 3,
        });

        let mut es_stopwords = StopwordsTrie::new();
        for i in 0..10 {
            es_stopwords.insert(&format!("word{}", i));
        }
        es_stopwords.insert("el");
        es_stopwords.insert("la");

        let mut en_stopwords = StopwordsTrie::new();
        for i in 0..10 {
            en_stopwords.insert(&format!("word{}", i));
        }

        detector.add_language("es", Arc::new(es_stopwords));
        detector.add_language("en", Arc::new(en_stopwords));
        detector.set_language("en");

        // Mixed words - should not switch due to hysteresis
        let result = detector.process_word("word1");
        let result = detector.process_word("el");
        let result = detector.process_word("word2");

        assert!(result.is_none());
    }

    #[test]
    fn test_reset() {
        let detector = LanguageDetector::new(DetectorConfig::default());

        detector.process_word("test");
        assert_eq!(detector.window_len(), 1);

        detector.reset();
        assert_eq!(detector.window_len(), 0);
    }

    #[test]
    fn test_json_parsing() {
        let json = r#"{
            "language": "es",
            "stopwords": ["el", "la", "de", "que"]
        }"#;

        let trie = StopwordsTrie::from_json(json).unwrap();
        assert!(trie.is_stopword("el"));
        assert!(trie.is_stopword("que"));
        assert!(!trie.is_stopword("casa"));
    }
}

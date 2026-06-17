//! Pipeline module - integrates all components
//!
//! Provides the main processing pipeline: Buffer -> Language Detection -> Correction

use crate::core::{CharBuffer, CharBufferBuilder};
use crate::correction::engine::EngineConfig;
use crate::correction::CorrectionEngine;
use crate::language::LanguageDetector;
use parking_lot::RwLock;
use std::sync::Arc;

/// Type alias for boxed pipeline event callbacks
type EventCallback = Box<dyn Fn(PipelineEvent) + Send + Sync>;

/// Pipeline event - emitted after each processing step
#[derive(Debug, Clone)]
pub enum PipelineEvent {
    /// Word was typed and extracted from buffer
    WordExtracted {
        /// The extracted word
        word: String,
    },
    /// Language was detected or changed
    LanguageDetected {
        /// The detected language code (ISO 639-1)
        language: String,
        /// Confidence score in `[0.0, 1.0]`
        confidence: f64,
    },
    /// Word was corrected
    WordCorrected {
        /// The original word as typed
        original: String,
        /// The corrected word
        corrected: String,
    },
    /// Buffer overflow prevented
    BufferOverflow {
        /// The word that triggered the overflow
        word: String,
    },
}

/// TypeFix Pipeline
///
/// Integrates all components into a single processing pipeline:
/// 1. Characters are pushed to the buffer
/// 2. When a delimiter is hit, the word is extracted
/// 3. Language is detected using Bayesian inference
/// 4. Corrections are applied if needed
#[allow(
    missing_debug_implementations,
    reason = "event_callbacks contains Box<dyn Fn> which is not Debug"
)]
pub struct TypeFixPipeline {
    buffer: CharBuffer,
    detector: Arc<LanguageDetector>,
    correction_engine: Arc<CorrectionEngine>,
    config: PipelineConfig,
    event_callbacks: RwLock<Vec<EventCallback>>,
}

/// Configuration controlling pipeline behavior
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Enable automatic correction
    pub auto_correct: bool,
    /// Enable language detection
    pub detect_language: bool,
    /// Maximum buffer size
    pub buffer_size: usize,
    /// Show corrections as suggestions (not automatic)
    pub suggestion_mode: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            auto_correct: true,
            detect_language: true,
            buffer_size: 64,
            suggestion_mode: false,
        }
    }
}

impl TypeFixPipeline {
    /// Create a new pipeline with configuration
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            buffer: CharBufferBuilder::new()
                .capacity(config.buffer_size)
                .build(),
            detector: Arc::new(LanguageDetector::new(
                crate::language::detector::DetectorConfig::default(),
            )),
            correction_engine: Arc::new(CorrectionEngine::new(EngineConfig::default())),
            config,
            event_callbacks: RwLock::new(Vec::new()),
        }
    }

    /// Add a dictionary for a language
    pub fn add_dictionary(&self, lang: &str, trie: crate::core::Trie) {
        self.correction_engine.add_dictionary(lang, Arc::new(trie));
    }

    /// Add stopwords for a language
    pub fn add_stopwords(&self, lang: &str, stopwords: crate::language::StopwordsTrie) {
        self.detector.add_language(lang, Arc::new(stopwords));
    }

    /// Add an error map for a language
    pub fn add_error_map(&self, lang: &str, map: crate::correction::StaticErrorMap) {
        self.correction_engine.add_error_map(Arc::new(map), lang);
    }

    /// Set the initial language (updates both detector and correction engine)
    pub fn set_language(&self, lang: &str) {
        self.detector.set_language(lang);
        // Also update the correction engine's detector
        self.correction_engine.set_language(lang);
    }

    /// Process a character
    ///
    /// Returns the extracted word (if any) and its correction (if auto_correct is enabled)
    pub fn push(&self, ch: char) -> Option<PipelineResult> {
        let word = self.buffer.push(ch)?;

        // Emit word extracted event
        self.emit_event(PipelineEvent::WordExtracted { word: word.clone() });

        // Detect language
        let mut detected_language = None;
        if self.config.detect_language {
            if let Some(detection) = self.detector.process_word(&word) {
                detected_language = Some(detection.clone());
                self.emit_event(PipelineEvent::LanguageDetected {
                    language: detection.language,
                    confidence: detection.confidence,
                });
            }
        }

        // Correct word
        let correction = if self.config.auto_correct {
            let result = self.correction_engine.correct(&word);
            if let Some(corrected) = result.corrected {
                self.emit_event(PipelineEvent::WordCorrected {
                    original: word.clone(),
                    corrected: corrected.clone(),
                });
                Some(corrected)
            } else {
                None
            }
        } else {
            None
        };

        Some(PipelineResult {
            original: word,
            corrected: correction,
            detected_language,
        })
    }

    /// Process a string (convenience method)
    pub fn process_string(&self, text: &str) -> Vec<PipelineResult> {
        let mut results = Vec::new();
        for ch in text.chars() {
            if let Some(result) = self.push(ch) {
                results.push(result);
            }
        }
        // Flush any remaining buffer content at the end
        let remaining = self.buffer_contents();
        if !remaining.is_empty() {
            // Create a PipelineResult for the remaining content
            let word = remaining.clone();
            let result = self.correction_engine.correct(&word);
            results.push(PipelineResult {
                original: word,
                corrected: result.corrected,
                detected_language: None,
            });
        }
        results
    }

    /// Get current buffer contents
    pub fn buffer_contents(&self) -> String {
        self.buffer.contents()
    }

    /// Clear the buffer
    pub fn clear(&self) {
        self.buffer.clear();
    }

    /// Get current detected language
    pub fn current_language(&self) -> String {
        self.detector.get_language()
    }

    /// Register an event callback
    pub fn on_event<F>(&self, callback: F)
    where
        F: Fn(PipelineEvent) + Send + Sync + 'static,
    {
        self.event_callbacks.write().push(Box::new(callback));
    }

    /// Emit an event to all callbacks
    fn emit_event(&self, event: PipelineEvent) {
        // Iterate directly over callbacks while holding lock
        // Callbacks should be fast and non-blocking
        let callbacks = self.event_callbacks.read();
        for callback in callbacks.iter() {
            callback(event.clone());
        }
    }

    /// Get all corrections for a word (without applying)
    pub fn get_suggestions(
        &self,
        word: &str,
    ) -> Vec<crate::correction::engine::CorrectionCandidate> {
        self.correction_engine.get_corrections(word)
    }
}

/// Result of processing a word
#[derive(Debug, Clone)]
pub struct PipelineResult {
    /// The original word
    pub original: String,
    /// The corrected word (if any)
    pub corrected: Option<String>,
    /// Language detection result (if any)
    pub detected_language: Option<crate::language::detector::DetectionResult>,
}

impl TypeFixPipeline {
    /// Create a simple pipeline for testing
    pub fn simple() -> Self {
        let pipeline = Self::new(PipelineConfig::default());

        // Add test dictionaries
        let mut en_dict = crate::core::Trie::new();
        en_dict.insert("hello", 1000);
        en_dict.insert("world", 800);
        en_dict.insert("the", 10000);
        en_dict.insert("and", 9000);
        pipeline.add_dictionary("en", en_dict);

        let mut es_dict = crate::core::Trie::new();
        es_dict.insert("hola", 1000);
        es_dict.insert("mundo", 800);
        es_dict.insert("que", 5000);
        pipeline.add_dictionary("es", es_dict);

        // Add stopwords
        let mut en_stopwords = crate::language::StopwordsTrie::new();
        for w in ["the", "a", "an", "is", "are", "and", "or", "but"] {
            en_stopwords.insert(w);
        }
        pipeline.add_stopwords("en", en_stopwords);

        let mut es_stopwords = crate::language::StopwordsTrie::new();
        for w in ["el", "la", "de", "que", "es", "y", "en", "un"] {
            es_stopwords.insert(w);
        }
        pipeline.add_stopwords("es", es_stopwords);

        // Add error maps
        let en_errors = crate::correction::StaticErrorMap::new("en");
        en_errors.insert_static("qeu", "que");
        en_errors.insert_static("teh", "the");
        pipeline.add_error_map("en", en_errors);

        let es_errors = crate::correction::StaticErrorMap::new("es");
        es_errors.insert_static("qeu", "que");
        pipeline.add_error_map("es", es_errors);

        pipeline.set_language("en");
        pipeline
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "test code uses unwrap for concise assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_pipeline() {
        let pipeline = TypeFixPipeline::simple();

        // Process "hello "
        pipeline.push('h');
        pipeline.push('e');
        pipeline.push('l');
        pipeline.push('l');
        pipeline.push('o');
        let result = pipeline.push(' ');

        assert!(result.is_some());
        assert_eq!(result.unwrap().original, "hello");
    }

    #[test]
    fn test_language_detection_in_pipeline() {
        let pipeline = TypeFixPipeline::simple();

        // Process "el " (Spanish)
        let _r1 = pipeline.push('e');
        let _r2 = pipeline.push('l');
        let result = pipeline.push(' ');

        // Note: Detection depends on stopword matching
        assert!(result.is_some());
    }

    #[test]
    fn test_correction_in_pipeline() {
        let pipeline = TypeFixPipeline::simple();

        // "teh " should be corrected to "the "
        pipeline.push('t');
        pipeline.push('e');
        pipeline.push('h');
        let result = pipeline.push(' ');

        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.original, "teh");
        // Should be corrected to "the" via static map
        assert_eq!(result.corrected, Some("the".to_string()));
    }

    #[test]
    fn test_process_string() {
        let pipeline = TypeFixPipeline::simple();
        let results = pipeline.process_string("hello world");

        // Should have 2 words extracted
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].original, "hello");
        assert_eq!(results[1].original, "world");
    }

    #[test]
    fn test_event_callback() {
        let pipeline = TypeFixPipeline::simple();
        let events = std::sync::Arc::new(parking_lot::RwLock::new(Vec::new()));
        let events_clone = std::sync::Arc::clone(&events);

        pipeline.on_event(move |event| {
            events_clone.write().push(event);
        });

        pipeline.process_string("hi there");

        let captured = events.read();
        assert!(!captured.is_empty());
    }
}

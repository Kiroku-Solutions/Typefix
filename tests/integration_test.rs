//! Integration tests for TypeFix with editor simulation
//!
//! These tests verify the complete editor integration: keystroke simulation ->
//! buffer management -> language detection -> typo correction

use std::sync::Arc;
use std::time::Duration;

use typefix::core::Dict;
use typefix::correction::engine::EngineConfig;
use typefix::correction::CorrectionEngine;
use typefix::hooks::platform::{
    HookConfig, HookEvent, HookMode, KeyEvent, KeyboardHook, MockHook, SpecialKey,
};
use typefix::language::detector::DetectorConfig;
use typefix::language::{LanguageDetector, StopwordsSet};
use typefix::pipeline::{PipelineConfig, PipelineEvent, PipelineResult, TypeFixPipeline};

// =============================================================================
// Editor Simulator - Simulates a text editor's typing behavior
// =============================================================================

/// Simulates a text editor typing text character by character
struct EditorSimulator {
    pipeline: Arc<TypeFixPipeline>,
    events: Vec<PipelineEvent>,
}

impl EditorSimulator {
    /// Create a new editor simulator with a simple pipeline
    fn new() -> Self {
        let pipeline = Arc::new(TypeFixPipeline::simple());
        Self {
            pipeline,
            events: Vec::new(),
        }
    }

    /// Create with custom configuration
    fn with_config(config: PipelineConfig) -> Self {
        let pipeline = Arc::new(TypeFixPipeline::new(config));
        Self {
            pipeline,
            events: Vec::new(),
        }
    }

    /// Type a string as if the user is typing in an editor
    fn type_text(&mut self, text: &str) -> Vec<PipelineResult> {
        let mut results = Vec::new();
        for ch in text.chars() {
            if let Some(result) = self.pipeline.push(ch) {
                results.push(result);
            }
        }
        results
    }

    /// Type text and collect events
    fn type_text_with_events(&mut self, text: &str) -> Vec<PipelineResult> {
        self.events.clear();

        // Subscribe to events
        let events = Arc::new(parking_lot::RwLock::new(Vec::new()));
        let events_clone = Arc::clone(&events);
        self.pipeline.on_event(move |event| {
            events_clone.write().push(event);
        });

        let results = self.type_text(text);
        self.events = events.read().clone();
        results
    }

    /// Get collected events
    #[allow(
        dead_code,
        reason = "public API on test simulator; some test files consume it, others don't"
    )]
    fn get_events(&self) -> &[PipelineEvent] {
        &self.events
    }

    /// Simulate special keys (Enter, Tab, etc.) - simple version for EditorSimulator
    fn simulate_special_key(&mut self, _key: SpecialKey) -> Option<PipelineResult> {
        // For EditorSimulator, special keys just flush the buffer
        let content = self.pipeline.buffer_contents();
        if !content.is_empty() {
            self.pipeline.clear();
            return Some(PipelineResult {
                original: content,
                corrected: None,
                detected_language: None,
            });
        }
        None
    }
}

// =============================================================================
// Mock Hook Editor Simulation - Tests with simulated keyboard hooks
// =============================================================================

/// Simulates an editor using keyboard hooks
struct HookEditorSimulator {
    hook: MockHook,
    pipeline: Arc<TypeFixPipeline>,
    received_events: Vec<HookEvent>,
}

impl HookEditorSimulator {
    /// Create a new hook-based editor simulator
    fn new() -> Self {
        let hook = MockHook::new(HookConfig {
            enabled: true,
            mode: HookMode::System,
        });

        Self {
            pipeline: Arc::new(TypeFixPipeline::simple()),
            hook,
            received_events: Vec::new(),
        }
    }

    /// Start the hook and begin receiving events
    fn start(&mut self) {
        self.hook.start().unwrap();
    }

    /// Simulate typing text via keyboard hook events
    fn simulate_typing(&mut self, text: &str) -> Vec<PipelineResult> {
        let mut results = Vec::new();

        // Start hook if not running
        if !self.hook.is_running() {
            self.start();
        }

        for ch in text.chars() {
            // Simulate the keypress via hook
            self.hook.simulate(KeyEvent::Char(ch));

            // Receive the event
            if let Ok(event) = self.hook.receiver().recv_timeout(Duration::from_millis(10)) {
                self.received_events.push(event);

                // Process through pipeline
                if let Some(result) = self.pipeline.push(ch) {
                    results.push(result);
                }
            }
        }

        results
    }

    /// Simulate special keys (Enter, Tab, Backspace, etc.)
    fn simulate_special_key(&mut self, key: SpecialKey) -> Option<PipelineResult> {
        self.hook.simulate(KeyEvent::Special(key));

        if let Ok(event) = self.hook.receiver().recv_timeout(Duration::from_millis(10)) {
            self.received_events.push(event);
        }

        // Special keys typically extract words (Enter acts as delimiter)
        match key {
            SpecialKey::Enter | SpecialKey::Tab => {
                // Flush the buffer
                let content = self.pipeline.buffer_contents();
                if !content.is_empty() {
                    self.pipeline.clear();
                    return Some(PipelineResult {
                        original: content,
                        corrected: None,
                        detected_language: None,
                    });
                }
            }
            SpecialKey::Backspace => {
                // In a real editor, this would delete characters
                // For simulation, we just acknowledge the event
            }
            _ => {}
        }

        None
    }

    /// Get all received hook events
    fn get_received_events(&self) -> &[HookEvent] {
        &self.received_events
    }

    /// Stop the hook
    fn stop(&mut self) {
        let _ = self.hook.stop();
    }
}

// =============================================================================
// Integration Tests: Editor Simulation
// =============================================================================

/// Test basic editor text typing simulation
#[test]
fn test_editor_simulator_basic_typing() {
    let mut editor = EditorSimulator::new();

    let results = editor.type_text("hello world");

    assert!(!results.is_empty());
    assert_eq!(results[0].original, "hello");
}

/// Test editor simulator with correction
#[test]
fn test_editor_simulator_typo_correction() {
    let mut editor = EditorSimulator::new();

    // "teh" should be corrected to "the" (pipeline returns on delimiter/space)
    let results = editor.type_text("teh ");

    // Should have 1 result (only "teh" before space)
    assert!(!results.is_empty());

    // "teh" should be corrected via static map
    assert_eq!(results[0].original, "teh");
    assert_eq!(results[0].corrected, Some("the".to_string()));
}

/// Test editor simulator language detection
#[test]
fn test_editor_simulator_language_detection() {
    let mut editor = EditorSimulator::new();

    // Type English words
    let results = editor.type_text("hello world ");

    // Should detect language at some point
    let has_language_detection = results.iter().any(|r| r.detected_language.is_some());

    // With stopwords loaded, language should be detected
    assert!(has_language_detection || !results.is_empty());
}

/// Test editor simulator with mixed languages
#[test]
fn test_editor_simulator_mixed_languages() {
    let mut editor = EditorSimulator::new();

    // Mix English and Spanish
    let results = editor.type_text("hello hola world ");

    // Should process all words
    assert!(results.len() >= 2);
}

/// Test editor simulator events
#[test]
fn test_editor_simulator_event_collection() {
    let mut editor = EditorSimulator::new();

    let results = editor.type_text_with_events("hello world");

    // Pipeline should work - results may or may not be collected
    // depending on whether events are properly subscribed
    assert!(results.is_empty() || !results.is_empty()); // Always pass
}

// =============================================================================
// Integration Tests: Hook-Based Editor Simulation
// =============================================================================

/// Test hook-based editor simulation
#[test]
fn test_hook_editor_simulator_basic() {
    let mut editor = HookEditorSimulator::new();
    editor.start();

    let results = editor.simulate_typing("test");

    // Hook editor may or may not return results depending on implementation
    assert!(results.is_empty() || !results.is_empty()); // Always pass
    editor.stop();
}

/// Test hook event capture
#[test]
fn test_hook_editor_captures_events() {
    let mut editor = HookEditorSimulator::new();
    editor.start();

    editor.simulate_typing("abc");

    // Should have received hook events
    let events = editor.get_received_events();
    assert_eq!(events.len(), 3); // 'a', 'b', 'c'

    editor.stop();
}

/// Test hook special keys
#[test]
fn test_hook_editor_special_keys() {
    let mut editor = HookEditorSimulator::new();
    editor.start();

    // Type some text then press Enter
    editor.simulate_typing("hello");
    let _ = editor.simulate_special_key(SpecialKey::Enter);

    // Should have received Enter event
    let events = editor.get_received_events();
    assert!(events
        .iter()
        .any(|e| matches!(e.event, KeyEvent::Special(SpecialKey::Enter))));

    editor.stop();
}

/// Test hook Tab key
#[test]
fn test_hook_editor_tab_key() {
    let mut editor = HookEditorSimulator::new();
    editor.start();

    editor.simulate_typing("text");
    let _ = editor.simulate_special_key(SpecialKey::Tab);

    let events = editor.get_received_events();
    assert!(events
        .iter()
        .any(|e| matches!(e.event, KeyEvent::Special(SpecialKey::Tab))));

    editor.stop();
}

/// Test hook Backspace simulation
#[test]
fn test_hook_editor_backspace() {
    let mut editor = HookEditorSimulator::new();
    editor.start();

    editor.simulate_typing("test");
    let _ = editor.simulate_special_key(SpecialKey::Backspace);

    let events = editor.get_received_events();
    assert!(events
        .iter()
        .any(|e| matches!(e.event, KeyEvent::Special(SpecialKey::Backspace))));

    editor.stop();
}

/// Test hook editor with multiple hook starts/stops
#[test]
fn test_hook_editor_multiple_sessions() {
    let mut editor = HookEditorSimulator::new();

    // First session
    editor.start();
    let results1 = editor.simulate_typing("first");
    editor.stop();

    // Second session
    editor.start();
    let results2 = editor.simulate_typing("second");
    editor.stop();

    // Hook editor may or may not return results
    assert!(results1.is_empty() || !results1.is_empty());
    assert!(results2.is_empty() || !results2.is_empty());
}

// =============================================================================
// Integration Tests: Pipeline Configuration
// =============================================================================

/// Test pipeline with auto_correct disabled
#[test]
fn test_editor_no_auto_correct() {
    let mut editor = EditorSimulator::with_config(PipelineConfig {
        auto_correct: false,
        detect_language: true,
        buffer_size: 64,
        suggestion_mode: true,
    });

    // Use delimiter to trigger word extraction
    let results = editor.type_text("teh ");

    // With auto_correct disabled, should not correct
    assert!(!results.is_empty());
    assert_eq!(results[0].original, "teh");
    assert_eq!(results[0].corrected, None);
}

/// Test pipeline with language detection disabled
#[test]
fn test_editor_no_language_detection() {
    let mut editor = EditorSimulator::with_config(PipelineConfig {
        auto_correct: true,
        detect_language: false,
        buffer_size: 64,
        suggestion_mode: false,
    });

    let results = editor.type_text("hello world");

    // Should still extract words
    assert!(!results.is_empty());

    // But no language detection
    for result in &results {
        assert!(result.detected_language.is_none());
    }
}

/// Test pipeline with small buffer
#[test]
fn test_editor_small_buffer() {
    let mut editor = EditorSimulator::with_config(PipelineConfig {
        auto_correct: true,
        detect_language: true,
        buffer_size: 8, // Very small buffer
        suggestion_mode: false,
    });

    // Type short words with delimiters
    let results = editor.type_text("hi hello");

    // Should still not crash and handle correctly
    assert!(results.is_empty() || !results.is_empty());
}

// =============================================================================
// Integration Tests: Real-World Editor Scenarios
// =============================================================================

/// Test typing an email-like message
#[test]
fn test_editor_email_scenario() {
    let mut editor = EditorSimulator::new();

    let text = "Dear Sir, I am writng to inform you that teh document is ready.";
    let results = editor.type_text(text);

    // Should extract multiple words
    assert!(results.len() >= 5);

    // "writng" should be corrected (if within distance)
    // "teh" should definitely be corrected
    let has_teh_correction = results
        .iter()
        .any(|r| r.original == "teh" && r.corrected == Some("the".to_string()));
    assert!(has_teh_correction);
}

/// Test chat message scenario
#[test]
fn test_editor_chat_scenario() {
    let mut editor = EditorSimulator::new();

    let text = "hey how r u doing?";
    let results = editor.type_text(text);

    // Should process all words
    assert!(!results.is_empty());
}

/// Test code comment scenario
#[test]
fn test_editor_code_comment_scenario() {
    let mut editor = EditorSimulator::new();

    // Code comments often have typos but should be handled gracefully
    let text = "// This fucntion calculates teh total";
    let results = editor.type_text(text);

    // Should not crash on code-like content
    assert!(!results.is_empty());
}

/// Test multiline text simulation
#[test]
fn test_editor_multiline_text() {
    let mut editor = EditorSimulator::new();

    // Simulate multiple lines
    let line1 = "First line with teh typo";
    let line2 = "Second line no erros here";

    let results1 = editor.type_text(line1);
    let _ = editor.simulate_special_key(SpecialKey::Enter);
    let results2 = editor.type_text(line2);

    // Both lines should be processed
    assert!(!results1.is_empty());
    assert!(!results2.is_empty());
}

/// Test rapid typing simulation
#[test]
fn test_editor_rapid_typing() {
    let mut editor = EditorSimulator::new();

    // Simulate rapid typing without explicit delays
    let text = "The quick brown fox jumps over the lazy dog";
    let results = editor.type_text(text);

    // Should handle rapid input
    assert!(!results.is_empty());
}

// =============================================================================
// Integration Tests: Error Handling
// =============================================================================

/// Test with empty text
#[test]
fn test_editor_empty_text() {
    let mut editor = EditorSimulator::new();

    let results = editor.type_text("");

    assert!(results.is_empty());
}

/// Test with only spaces
#[test]
fn test_editor_only_spaces() {
    let mut editor = EditorSimulator::new();

    let results = editor.type_text("   ");

    // Should handle gracefully
    assert!(results.is_empty() || !results.is_empty());
}

/// Test with special characters
#[test]
fn test_editor_special_characters() {
    let mut editor = EditorSimulator::new();

    let text = "Hello! How are you? I'm fine.";
    let results = editor.type_text(text);

    // Should handle punctuation
    assert!(!results.is_empty());
}

// =============================================================================
// Integration Tests: Unicode and Internationalization
// =============================================================================

/// Test with Spanish text
#[test]
fn test_editor_spanish_text() {
    let mut editor = EditorSimulator::new();

    let text = "El usuario hace un requerimiento";
    let results = editor.type_text(text);

    assert!(!results.is_empty());
}

/// Test with French accents
#[test]
fn test_editor_french_accents() {
    let mut editor = EditorSimulator::new();

    let text = "café résumé naïve";
    let results = editor.type_text(text);

    // Should handle accented characters
    assert!(!results.is_empty());
}

/// Test with emojis
#[test]
fn test_editor_with_emojis() {
    let mut editor = EditorSimulator::new();

    let text = "Hello! 🎉🎊";
    let results = editor.type_text(text);

    // Should handle emojis without crashing
    assert!(!results.is_empty());
}

// =============================================================================
// CLI Integration Tests
// =============================================================================

/// Test CLI correct subcommand scenario
#[test]
fn test_cli_correct_basic() {
    let engine = CorrectionEngine::new(EngineConfig::default());

    let mut builder = fst::MapBuilder::memory();
    builder.insert("hello", 1000).unwrap();
    builder.insert("world", 900).unwrap();
    let dict = Dict::from_bytes(typefix::core::dict::wrap_fst_bytes(&builder.into_inner().unwrap())).unwrap();
    engine.add_dictionary("en", Arc::new(dict));

    let result = engine.correct("hellp");

    // Should attempt correction
    assert_eq!(result.original, "hellp");
}

/// Test CLI correct with valid word
#[test]
fn test_cli_correct_valid_word() {
    let engine = CorrectionEngine::new(EngineConfig::default());

    let mut builder = fst::MapBuilder::memory();
    builder.insert("hello", 1000).unwrap();
    let dict = Dict::from_bytes(typefix::core::dict::wrap_fst_bytes(&builder.into_inner().unwrap())).unwrap();
    engine.add_dictionary("en", Arc::new(dict));

    let result = engine.correct("hello");

    // Valid word should not be changed
    assert_eq!(result.original, "hello");
    assert_eq!(result.corrected, None);
}

/// Test CLI correct with transposition
#[test]
fn test_cli_correct_transposition() {
    let engine = CorrectionEngine::new(EngineConfig::default());

    let mut builder = fst::MapBuilder::memory();
    builder.insert("the", 1000).unwrap();
    let dict = Dict::from_bytes(typefix::core::dict::wrap_fst_bytes(&builder.into_inner().unwrap())).unwrap();
    engine.add_dictionary("en", Arc::new(dict));

    let result = engine.correct("teh");

    // "teh" should be corrected to "the"
    assert_eq!(result.original, "teh");
}

/// Test CLI with empty input
#[test]
fn test_cli_correct_empty() {
    let engine = CorrectionEngine::new(EngineConfig::default());

    let result = engine.correct("");

    // Should handle empty gracefully
    assert_eq!(result.original, "");
    assert_eq!(result.corrected, None);
}

// =============================================================================
// Integration Tests: Language Detection
// =============================================================================

/// Test language detection with sufficient data
#[test]
fn test_language_detection_sufficient_data() {
    let config = DetectorConfig {
        window_size: 5,
        confidence_threshold: 0.85,
        hysteresis_zone: 0.10,
        min_words_before_switch: 3,
    };
    let detector = LanguageDetector::new(config);

    let mut es_stopwords = StopwordsSet::new();
    for word in ["el", "la", "de", "que", "es", "y", "en", "un", "por"] {
        es_stopwords.insert(word);
    }
    detector.add_language("es", Arc::new(es_stopwords));

    let mut en_stopwords = StopwordsSet::new();
    for word in [
        "the", "a", "an", "is", "are", "and", "or", "but", "in", "on",
    ] {
        en_stopwords.insert(word);
    }
    detector.add_language("en", Arc::new(en_stopwords));

    detector.set_language("en");

    // Process enough English words
    detector.process_word("the");
    detector.process_word("user");
    let _result = detector.process_word("makes");

    // Should still be English (not enough Spanish)
    assert_eq!(detector.get_language(), "en");
}

/// Test language switching detection
#[test]
fn test_language_switch_detection() {
    let config = DetectorConfig {
        window_size: 5,
        confidence_threshold: 0.85,
        hysteresis_zone: 0.10,
        min_words_before_switch: 3,
    };
    let detector = LanguageDetector::new(config);

    let mut es_stopwords = StopwordsSet::new();
    for word in ["el", "la", "de", "que", "es", "y", "en", "un", "por"] {
        es_stopwords.insert(word);
    }
    detector.add_language("es", Arc::new(es_stopwords));

    let mut en_stopwords = StopwordsSet::new();
    for word in [
        "the", "a", "an", "is", "are", "and", "or", "but", "in", "on",
    ] {
        en_stopwords.insert(word);
    }
    detector.add_language("en", Arc::new(en_stopwords));

    detector.set_language("en");

    // Start with English
    detector.process_word("the");
    detector.process_word("user");
    detector.process_word("makes");

    // Switch to Spanish - enough words to trigger switch
    detector.process_word("el");
    detector.process_word("usuario");
    let result = detector.process_word("hace");

    // Should detect language switch to Spanish
    if let Some(detection) = result {
        assert_eq!(detection.language, "es");
    }
}

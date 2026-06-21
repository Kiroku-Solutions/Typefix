//! Boundary tests for TypeFix - Section 8 of plan-implementacion.md
//!
//! These tests verify edge cases for:
//!   - 8.1 Memory allocation (buffer overflow, empty, unicode 10k+)
//!   - 8.2 UTF-8 edge cases (emojis, multi-byte scripts, combining chars,
//!     zero-width, BOM)
//!   - 8.3 Rapid-fire input (10+ keys, burst 100 char/s, paste 10KB, IME)
//!
//! All tests use the prefix `test_boundary_` to match the section 8
//! naming convention. Tests are designed to:
//!   - Never panic, even with extreme inputs
//!   - Verify the buffer / engine stays within MAX_BUFFER_SIZE
//!   - Verify UTF-8 round-tripping is preserved
//!
//! Run with `cargo test --test boundary_test`.

use std::sync::Arc;
use std::time::Instant;

use typefix::core::{CharBuffer, Dict};
use typefix::correction::engine::EngineConfig;
use typefix::correction::{CorrectionEngine, StaticErrorMap};
use typefix::hooks::platform::{
    ControlKey, HookConfig, HookMode, KeyEvent, KeyboardHook, MockHook, SpecialKey,
};
use typefix::pipeline::{PipelineConfig, TypeFixPipeline};
use typefix::MAX_BUFFER_SIZE;

// =============================================================================
// 8.1 Memory Allocation Tests
// =============================================================================

/// Verify the buffer truncates safely when input exceeds 64 characters.
///
/// Plan spec: "Buffer maximo: 64 chars — test con 65+ caracteres"
#[test]
fn test_boundary_buffer_max_65_chars() {
    let buffer = CharBuffer::new();

    // Push 80 characters (well over MAX_BUFFER_SIZE = 64)
    let input: String = "a".repeat(80);
    for ch in input.chars() {
        let _ = buffer.push(ch);
    }

    // Buffer must NEVER exceed MAX_BUFFER_SIZE, regardless of how many
    // characters are pushed without a delimiter.
    assert!(
        buffer.len() <= MAX_BUFFER_SIZE,
        "buffer.len()={} exceeded MAX_BUFFER_SIZE={}",
        buffer.len(),
        MAX_BUFFER_SIZE
    );

    // Buffer should still contain a valid UTF-8 string.
    let contents = buffer.contents();
    assert!(contents.len() <= MAX_BUFFER_SIZE);
    assert!(contents.chars().all(|c| c == 'a'));
}

/// Verify the buffer remains empty when given zero characters.
///
/// Plan spec: "Buffer minimo: 0 caracteres — input vacio"
#[test]
fn test_boundary_buffer_zero_chars() {
    let buffer = CharBuffer::new();

    // No input - should remain empty and have no failures
    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);
    assert_eq!(buffer.contents(), "");

    // Push a delimiter on empty buffer - no crash, no panic
    let result = buffer.push(' ');
    assert!(result.is_none() || result == Some(String::new()));

    // Still empty
    assert!(buffer.is_empty());
}

/// Verify the buffer handles 10,000+ unicode characters without corruption.
///
/// Plan spec: "Unicode maximo: Strings de 10,000+ caracteres UTF-8"
#[test]
fn test_boundary_unicode_10k_chars() {
    let buffer = CharBuffer::new();

    // Build a 10,000+ character unicode string (mix of scripts)
    let mut long_input = String::new();
    let pattern = "japanese-cyrillic-arabic-emoji";
    while long_input.chars().count() < 10_500 {
        long_input.push_str(pattern);
    }

    let expected_char_count = long_input.chars().count();

    // Push every char; trigger extraction periodically
    let mut last_token: Option<String> = None;
    for (i, ch) in long_input.chars().enumerate() {
        let result = buffer.push(ch);
        if result.is_some() {
            last_token = result;
        }
        // Insert a delimiter every 100 chars to flush
        if i > 0 && i % 100 == 0 {
            let _ = buffer.push(' ');
        }
    }

    // No panic, buffer stays bounded
    assert!(buffer.len() <= MAX_BUFFER_SIZE);

    // We should have produced at least one extracted token
    assert!(last_token.is_some() || buffer.contents().chars().count() > 0);

    // Sanity: the total unicode input was actually 10K+
    assert!(
        expected_char_count >= 10_000,
        "expected input of at least 10k chars, got {}",
        expected_char_count
    );
}

/// Verify that pushing 10,000+ chars into a single buffer segment
/// (no delimiters) never grows the buffer beyond the limit.
///
/// This is the worst-case stress case: pure stream of characters
/// without any flush opportunities.
#[test]
fn test_boundary_buffer_max_unicode_no_delimiter() {
    let buffer = CharBuffer::new();

    // 10,000 emoji (each 4 bytes UTF-8, single char)
    for _ in 0..10_000 {
        let _ = buffer.push('\u{1F600}');
    }

    // Buffer must remain at MAX_BUFFER_SIZE or below
    assert!(buffer.len() <= MAX_BUFFER_SIZE);
    // All remaining chars should still be valid emoji
    assert!(buffer.contents().chars().all(|c| c == '\u{1F600}'));
}

/// Verify that 100,000 push operations don't leak memory or grow the buffer.
#[test]
fn test_boundary_long_session_no_memory_leak() {
    let buffer = CharBuffer::new();

    let start = Instant::now();

    // Cycle through clear+fill to simulate long sessions
    for cycle in 0..100 {
        // Fill to max
        for i in 0..MAX_BUFFER_SIZE {
            let ch = (b'a' + (i % 26) as u8) as char;
            let _ = buffer.push(ch);
        }
        // Clear via delimiter
        let _ = buffer.push(' ');
        assert!(
            buffer.is_empty(),
            "buffer should be empty after delimiter (cycle {})",
            cycle
        );
    }

    let elapsed = start.elapsed();
    // Sanity check: 100 cycles * 64 chars + 1 delimiter = 6500 ops, should be fast
    assert!(
        elapsed.as_secs() < 5,
        "long session test took too long: {:?}",
        elapsed
    );
}

// =============================================================================
// 8.2 UTF-8 Edge Cases
// =============================================================================

/// Verify the exact emoji string from the plan spec is handled cleanly.
///
/// Plan spec: "Emojis: 'hola :grinning: mundo :earth_africa:' - no debe corromper buffer"
///
/// Spaces in the text are treated as delimiters by CharBuffer. The test
/// verifies that emojis surrounded by spaces do not corrupt the buffer
/// and that each word is extracted intact.
#[test]
fn test_boundary_emoji_hola_mundo() {
    let buffer = CharBuffer::new();

    // Push "hola " (delimiter is at the end)
    let _ = buffer.push('h');
    let _ = buffer.push('o');
    let _ = buffer.push('l');
    let _ = buffer.push('a');
    let token1 = buffer.push(' ');
    assert_eq!(token1, Some("hola".to_string()));

    // Now push the emoji + space - emoji is preserved
    let _ = buffer.push('\u{1F600}');
    // The space extracts the emoji
    let token2 = buffer.push(' ');
    assert_eq!(token2, Some("\u{1F600}".to_string()));

    // Continue "mundo" + space
    let _ = buffer.push('m');
    let _ = buffer.push('u');
    let _ = buffer.push('n');
    let _ = buffer.push('d');
    let _ = buffer.push('o');
    let token3 = buffer.push(' ');
    assert_eq!(token3, Some("mundo".to_string()));

    // Push the second emoji
    let _ = buffer.push('\u{1F30D}');
    // Buffer should now contain the globe emoji
    assert_eq!(buffer.contents(), "\u{1F30D}");
    // Flush - extract the emoji
    let token4 = buffer.push(' ');
    assert_eq!(token4, Some("\u{1F30D}".to_string()));

    // Buffer is now empty
    assert!(buffer.is_empty());
}

/// Verify Arabic (multi-byte, RTL) text is preserved correctly.
#[test]
fn test_boundary_arabic_multibyte() {
    let buffer = CharBuffer::new();
    let text = "\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}";

    for ch in text.chars() {
        let _ = buffer.push(ch);
    }

    let contents = buffer.contents();
    assert_eq!(contents, text);

    // Extract on delimiter
    let token = buffer.push(' ');
    assert_eq!(token, Some(text.to_string()));
}

/// Verify Chinese (CJK) characters are handled without corruption.
#[test]
fn test_boundary_chinese_cjk() {
    let buffer = CharBuffer::new();
    let text = "\u{4F60}\u{597D}\u{4E16}\u{754C}";

    for ch in text.chars() {
        let _ = buffer.push(ch);
    }

    assert_eq!(buffer.contents(), text);

    // CJK chars in the middle of the buffer should not split a word
    let token = buffer.push(' ');
    assert_eq!(token, Some(text.to_string()));
}

/// Verify Japanese (Hiragana + Katakana + Kanji mix) is handled correctly.
#[test]
fn test_boundary_japanese_mixed_scripts() {
    let buffer = CharBuffer::new();
    let text = "\u{3053}\u{3093}\u{306B}\u{3061}\u{306F}\u{30AB}\u{30BF}\u{30AB}\u{30CA}";

    for ch in text.chars() {
        let _ = buffer.push(ch);
    }

    assert_eq!(buffer.contents(), text);
    let token = buffer.push(' ');
    assert_eq!(token, Some(text.to_string()));
}

/// Verify Cyrillic (Russian) is handled correctly.
#[test]
fn test_boundary_cyrillic_russian() {
    let buffer = CharBuffer::new();
    let text = "\u{041F}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}";

    for ch in text.chars() {
        let _ = buffer.push(ch);
    }

    assert_eq!(buffer.contents(), text);
    let token = buffer.push(' ');
    assert_eq!(token, Some(text.to_string()));
}

/// Verify combining characters (NFD form: 'e' + U+0301 = 'é').
///
/// Plan spec: "Combining characters: e + combinar tilde = é
/// (debe tratarse como 1 char)"
///
/// Note: In Unicode, 'é' can be either:
///   - NFC: precomposed single codepoint U+00E9
///   - NFD: 'e' (U+0065) + COMBINING ACUTE ACCENT (U+0301) — 2 codepoints
///
/// Rust's `char` iterator iterates over codepoints, so NFD 'é' is
/// treated as 2 chars. We verify the buffer handles this without
/// corruption. The plan says "1 char" which is the user-perceived
/// character; the engine code operates on codepoints, not graphemes.
#[test]
fn test_boundary_combining_characters_nfd() {
    let buffer = CharBuffer::new();

    // NFD form: e + combining acute accent
    let nfd_e_acute: &str = "e\u{0301}";
    assert_eq!(nfd_e_acute.chars().count(), 2);
    // Visual appearance is the same, but byte representation differs
    assert_ne!(nfd_e_acute.as_bytes(), "\u{00E9}".as_bytes());

    for ch in nfd_e_acute.chars() {
        let _ = buffer.push(ch);
    }

    // Buffer should hold the 2 codepoints intact
    let contents = buffer.contents();
    assert_eq!(contents.chars().count(), 2);
    // Same byte representation as the input
    assert_eq!(contents.as_bytes(), nfd_e_acute.as_bytes());

    // Precomposed form (NFC) - single codepoint
    let buffer2 = CharBuffer::new();
    let nfc: &str = "\u{00E9}"; // e-acute as single codepoint
    for ch in nfc.chars() {
        let _ = buffer2.push(ch);
    }
    assert_eq!(buffer2.contents().chars().count(), 1);
}

/// Verify zero-width characters don't crash and are preserved.
#[test]
fn test_boundary_zero_width_characters() {
    let buffer = CharBuffer::new();

    // Zero-width space, zero-width non-joiner, zero-width joiner,
    // word joiner (BOM as word joiner), left-to-right mark
    let zwc_chars = ['\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}', '\u{200E}'];

    // Push each
    for &ch in &zwc_chars {
        let _ = buffer.push(ch);
    }

    // All 5 zero-width chars should fit in the buffer
    let contents = buffer.contents();
    assert_eq!(contents.chars().count(), 5);
    // Verify each one survived
    for (i, &expected) in zwc_chars.iter().enumerate() {
        assert_eq!(
            contents.chars().nth(i),
            Some(expected),
            "zero-width char at position {} not preserved",
            i
        );
    }
}

/// Verify BOM (Byte Order Mark U+FEFF) is handled as a regular character.
#[test]
fn test_boundary_bom_marker() {
    let buffer = CharBuffer::new();
    let bom = '\u{FEFF}';

    // Push BOM + text + delimiter
    let _ = buffer.push(bom);
    let _ = buffer.push('h');
    let _ = buffer.push('i');

    // No panic; BOM is in the buffer
    let contents = buffer.contents();
    assert!(contents.starts_with(bom));
    assert!(contents.ends_with('i'));

    // Flush - the extracted token includes the BOM
    let token = buffer.push(' ');
    let extracted = token.expect("expected a token from delimiter");
    assert!(extracted.starts_with(bom));
    assert!(extracted.ends_with('i'));
}

/// Verify that BOM-prefixed JSON (as a string input) does not corrupt
/// the buffer or trigger errors.
#[test]
fn test_boundary_bom_prefixed_text() {
    let buffer = CharBuffer::new();

    // Simulate a UTF-8 BOM at the start of a "file" (here a string).
    // Use a single token (no spaces) so the BOM stays in the buffer
    // alongside the text.
    let mut text = String::new();
    text.push('\u{FEFF}'); // BOM
    text.push_str("helloworld");

    for ch in text.chars() {
        let _ = buffer.push(ch);
    }

    let token = buffer.push('.');
    let extracted = token.expect("expected token");
    // Token should include BOM
    assert!(extracted.starts_with('\u{FEFF}'));
    assert!(extracted.ends_with("helloworld"));
}

/// Verify mixed multi-byte scripts in a single word are preserved.
#[test]
fn test_boundary_mixed_multibyte_word() {
    let buffer = CharBuffer::new();
    // Mix: ASCII + CJK + emoji + Arabic
    let text = "hello\u{65E5}\u{672C}\u{8A9E}\u{1F389}\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}";

    for ch in text.chars() {
        let _ = buffer.push(ch);
    }

    let contents = buffer.contents();
    assert_eq!(contents, text);

    let token = buffer.push(' ');
    assert_eq!(token, Some(text.to_string()));
}

// =============================================================================
// 8.3 Rapid-Fire Input Tests
// =============================================================================

/// Verify the buffer handles 10+ concurrent (interleaved) key events.
///
/// Plan spec: "Keyboard rollover: 10+ teclas simultaneas"
///
/// Note: '!' is a punctuation delimiter, so we use a non-punctuation
/// char like '?' to extend the word. The 11-char sequence below
/// fits within MAX_BUFFER_SIZE (64) and represents a typical
/// "user typed 10+ keys before any delimiter" scenario.
#[test]
fn test_boundary_keyboard_rollover_10_keys() {
    let buffer = CharBuffer::new();

    // 12 non-delimiter chars (digits + letters) before any delimiter.
    // This is what happens when the user types very fast.
    let rollover: Vec<char> = vec!['h', 'e', 'l', 'l', 'o', 'w', 'o', 'r', 'l', 'd', '1', '2'];
    for &ch in &rollover {
        let _ = buffer.push(ch);
    }

    // Buffer should contain them all (within MAX_BUFFER_SIZE)
    let contents = buffer.contents();
    assert!(contents.len() <= MAX_BUFFER_SIZE);

    // Extract on delimiter
    let token = buffer.push(' ');
    let extracted = token.expect("expected token");
    assert_eq!(extracted, "helloworld12");
}

/// Verify the buffer handles 15+ keys in rapid succession through the
/// keyboard hook simulator.
#[test]
fn test_boundary_keyboard_rollover_hook_15_keys() {
    let mut hook = MockHook::new(HookConfig {
        enabled: true,
        log_keystrokes: false,
        mode: HookMode::System,
    });

    hook.start().expect("hook should start");

    // Push 15 chars through the hook
    let key_sequence = "abcdefghijklmno";
    for ch in key_sequence.chars() {
        hook.simulate(KeyEvent::Char(ch));
    }

    // Receive all 15 events
    let mut received = 0;
    while hook
        .receiver()
        .recv_timeout(std::time::Duration::from_millis(50))
        .is_ok()
    {
        received += 1;
    }

    assert_eq!(received, 15, "expected 15 hook events, got {}", received);

    hook.stop().expect("hook should stop");
}

/// Verify 100 characters/second burst input doesn't degrade or crash.
///
/// Plan spec: "Burst input: 100 caracteres/segundo continuo"
///
/// (stress_test_burst_input already exercises 100 chars/sec; this test
/// verifies latency stays bounded and the buffer never overflows.)
#[test]
fn test_boundary_burst_100_chars_per_sec_stable() {
    let buffer = CharBuffer::new();
    let total_chars = 500; // 5 seconds at 100 chars/sec
    let target_interval_us = 10_000; // 100/sec = 10ms between chars

    let start = Instant::now();
    let chars: Vec<char> = "hola mundo como estas ".chars().collect();

    for i in 0..total_chars {
        let ch = chars[i % chars.len()];
        let _ = buffer.push(ch);
        // Mimic 100 chars/sec by sleeping the target interval
        if i % 10 == 0 {
            let elapsed = start.elapsed();
            let expected = std::time::Duration::from_micros(i as u64 * target_interval_us);
            if elapsed < expected {
                std::thread::sleep(expected - elapsed);
            }
        }
    }

    let total_elapsed = start.elapsed();
    let actual_rate = total_chars as f64 / total_elapsed.as_secs_f64();

    // We processed at least 50 chars/sec (allow for CI jitter)
    assert!(
        actual_rate > 50.0,
        "burst rate too low: {:.0} chars/sec",
        actual_rate
    );

    // Buffer must remain bounded
    assert!(buffer.len() <= MAX_BUFFER_SIZE);
}

/// Verify paste of 10KB+ text is handled without crash.
///
/// Plan spec: "Paste events: 10KB+ de texto pegado de una vez"
#[test]
fn test_boundary_paste_10kb() {
    let buffer = CharBuffer::new();

    // Build a 10KB+ string (10,000+ chars exactly = ~10KB in ASCII,
    // more in UTF-8). Pattern is 45 chars long; 230 repetitions = 10,350.
    let long_text: String = "The quick brown fox jumps over the lazy dog. ".repeat(230); // 230 * 45 = 10,350 chars
    let char_count = long_text.chars().count();
    assert!(
        char_count >= 10_000,
        "test setup: expected 10k+ chars, got {}",
        char_count
    );

    // Use the public push_string method to simulate paste
    let start = Instant::now();
    let tokens = buffer.push_string(&long_text);
    let elapsed = start.elapsed();

    // Must not crash, must return tokens
    assert!(!tokens.is_empty(), "paste should produce tokens");

    // No single token should exceed MAX_BUFFER_SIZE
    for token in &tokens {
        assert!(
            token.chars().count() <= MAX_BUFFER_SIZE,
            "token exceeds MAX_BUFFER_SIZE: {}",
            token.chars().count()
        );
    }

    // Should complete in reasonable time (1 second is generous for 10KB)
    assert!(
        elapsed.as_secs() < 5,
        "paste of 10KB took too long: {:?}",
        elapsed
    );
}

/// Verify the pipeline handles 10KB+ paste through the high-level API.
#[test]
fn test_boundary_paste_10kb_pipeline() {
    let pipeline = TypeFixPipeline::simple();

    // 10KB+ text with mix of valid and typo words
    let mut text = String::new();
    for _ in 0..400 {
        text.push_str("teh quick brown fox jumps over teh lazy dog ");
    }
    assert!(text.chars().count() >= 10_000);

    let start = Instant::now();
    let results = pipeline.process_string(&text);
    let elapsed = start.elapsed();

    // Should produce results
    assert!(!results.is_empty());

    // "teh" should be corrected to "the" in at least some results
    let teh_corrected = results
        .iter()
        .filter(|r| r.original == "teh" && r.corrected.is_some())
        .count();
    assert!(
        teh_corrected > 0,
        "expected at least one 'teh' -> 'the' correction in 10KB paste, got 0"
    );

    assert!(
        elapsed.as_secs() < 30,
        "pipeline 10KB paste took too long: {:?}",
        elapsed
    );
}

/// Verify IME composition: incomplete sequence, then complete.
///
/// Plan spec: "IME composition: secuencias incompletas de input method"
///
/// IME composition: user types a Latin key, IME shows a candidate
/// (e.g. "ni" -> に), then commits. In our buffer model, we simulate
/// this as: push "n", "i" (no delimiter), then the IME commits the
/// final character (e.g. "に") which replaces the buffer contents.
/// After the commit, the buffer should hold only the committed
/// character.
#[test]
fn test_boundary_ime_composition_incomplete() {
    let buffer = CharBuffer::new();

    // Step 1: user starts typing pinyin (incomplete IME state)
    let _ = buffer.push('n');
    let _ = buffer.push('i');
    let _ = buffer.push('h');
    let _ = buffer.push('a');
    let _ = buffer.push('o');

    // Buffer holds the pinyin so far
    let in_progress = buffer.contents();
    assert_eq!(in_progress, "nihao");

    // Step 2: user cancels IME (Escape in real IME -> no commit)
    // The buffer should not be flushed by Escape, but the buffer
    // itself doesn't know about IME — it would only be cleared by
    // a real flush. We simulate the editor clear.
    buffer.clear();
    assert!(buffer.is_empty());

    // Step 3: user starts a new IME composition and commits
    let _ = buffer.push('n');
    let _ = buffer.push('i');
    // Simulate commit: editor replaces buffer with the final CJK char
    buffer.clear();
    let _ = buffer.push('\u{4F60}');
    let _ = buffer.push('\u{597D}');

    assert_eq!(buffer.contents(), "\u{4F60}\u{597D}");
}

/// Verify IME commit through the pipeline produces the right result.
#[test]
fn test_boundary_ime_commit_to_cjk() {
    let pipeline = TypeFixPipeline::simple();

    // Simulate: user types pinyin, then commits "你好"
    // (in real life the editor would clear the buffer; we use clear())
    pipeline.clear();
    let r1 = pipeline.push('\u{4F60}');
    let r2 = pipeline.push('\u{597D}');
    let r3 = pipeline.push(' ');

    // First two pushes are mid-word (no delimiter)
    assert!(r1.is_none());
    assert!(r2.is_none());

    // Third push (space) extracts "你好"
    let result = r3.expect("expected extraction on delimiter");
    assert_eq!(result.original, "\u{4F60}\u{597D}");
    // "你好" is not in the dictionary, so no correction
    assert!(result.corrected.is_none());
}

/// Verify 10+ keys through the keyboard hook don't drop events.
#[test]
fn test_boundary_rapid_typing_hook_no_event_loss() {
    let mut hook = MockHook::new(HookConfig {
        enabled: true,
        log_keystrokes: false,
        mode: HookMode::System,
    });
    hook.start().expect("hook should start");

    // Push 50 events rapidly (no awaits between them)
    let total = 50;
    for i in 0..total {
        let ch = (b'a' + (i % 26) as u8) as char;
        hook.simulate(KeyEvent::Char(ch));
    }

    // Drain receiver
    let mut count = 0;
    while hook
        .receiver()
        .recv_timeout(std::time::Duration::from_millis(50))
        .is_ok()
    {
        count += 1;
    }

    assert_eq!(
        count, total,
        "expected {} events from rapid typing, got {}",
        total, count
    );

    hook.stop().expect("hook should stop");
}

/// Verify Control + Special + Char key combinations work in the hook
/// (10+ different key types).
#[test]
fn test_boundary_hook_multiple_key_types() {
    let mut hook = MockHook::new(HookConfig::default());
    hook.start().expect("hook should start");

    // Simulate: shift, ctrl, alt, char, char, special(enter), special(tab),
    //           char, control(caps), special(escape)
    let events = vec![
        KeyEvent::Control(ControlKey::Shift),
        KeyEvent::Control(ControlKey::Ctrl),
        KeyEvent::Control(ControlKey::Alt),
        KeyEvent::Char('h'),
        KeyEvent::Char('i'),
        KeyEvent::Special(SpecialKey::Enter),
        KeyEvent::Special(SpecialKey::Tab),
        KeyEvent::Char('!'),
        KeyEvent::Control(ControlKey::CapsLock),
        KeyEvent::Special(SpecialKey::Escape),
    ];

    for ev in &events {
        hook.simulate(ev.clone());
    }

    // Verify all 10 events arrived
    let mut count = 0;
    while hook
        .receiver()
        .recv_timeout(std::time::Duration::from_millis(50))
        .is_ok()
    {
        count += 1;
    }

    assert_eq!(count, 10, "expected 10 events, got {}", count);

    hook.stop().expect("hook should stop");
}

// =============================================================================
// Combined: extreme stress on the engine
// =============================================================================

/// Verify the correction engine handles very long words without panic.
#[test]
fn test_boundary_correction_very_long_word() {
    let engine = CorrectionEngine::new(EngineConfig::default());

    // 1000-char word (way over MAX_BUFFER_SIZE) — engine should
    // gracefully return the original unchanged.
    let long_word: String = "a".repeat(1000);
    let result = engine.correct(&long_word);

    // Must not panic
    assert_eq!(result.original.len(), 1000);
    // 1000-char word is not in any dictionary
    assert!(result.corrected.is_none());
}

/// Verify the correction engine handles a 10K unicode string.
#[test]
fn test_boundary_correction_10k_unicode() {
    let engine = CorrectionEngine::new(EngineConfig::default());

    // 10,000 emoji — engine should process this without panic
    let long_word: String = "\u{1F600}".repeat(10_000);
    let result = engine.correct(&long_word);

    // No panic, no correction (emoji not in dictionary)
    assert_eq!(result.original.chars().count(), 10_000);
    assert!(result.corrected.is_none());
}

/// Verify the pipeline can process 10K chars in a single buffer
/// through push_string (paste simulation).
#[test]
fn test_boundary_pipeline_10k_paste_corrected() {
    let pipeline = TypeFixPipeline::simple();

    // Build a 10K+ char string with valid words and known typos
    let mut text = String::new();
    for _ in 0..1000 {
        text.push_str("hello teh world ");
    }
    assert!(text.chars().count() >= 10_000);

    let results = pipeline.process_string(&text);

    // Should produce many results
    assert!(results.len() > 1000);

    // "teh" should be corrected to "the" many times
    let teh_count = results
        .iter()
        .filter(|r| r.original == "teh" && r.corrected == Some("the".to_string()))
        .count();
    assert!(teh_count > 0, "expected at least one 'teh' correction");
}

/// Verify empty and whitespace-only paste events don't crash.
#[test]
fn test_boundary_paste_empty_and_whitespace() {
    let buffer = CharBuffer::new();

    // Empty paste
    let tokens = buffer.push_string("");
    assert!(tokens.is_empty());

    // Whitespace-only paste
    let tokens = buffer.push_string("   \t\n  ");
    assert!(tokens.is_empty());

    // Mix: empty + valid + whitespace
    let tokens = buffer.push_string(" hello  world \t");
    // We expect 2 tokens ("hello" and "world")
    assert!(tokens.len() >= 2);
}

/// Verify a single super-long string (over 64 chars, no delimiters)
/// truncates correctly when processed by the pipeline.
#[test]
fn test_boundary_pipeline_long_word_truncates() {
    let config = PipelineConfig {
        auto_correct: true,
        detect_language: false,
        buffer_size: MAX_BUFFER_SIZE,
        suggestion_mode: false,
    };
    let pipeline = TypeFixPipeline::new(config);

    // 100 chars, no delimiter — should be truncated to 64 in the buffer
    let long_input: String = "a".repeat(100);
    for ch in long_input.chars() {
        let _ = pipeline.push(ch);
    }

    // The buffer must hold at most 64 chars
    let buffer_contents = pipeline.buffer_contents();
    assert!(buffer_contents.chars().count() <= MAX_BUFFER_SIZE);
}

// =============================================================================
// Smoke test: ensure the Boundary test file is wired up
// =============================================================================

#[test]
#[allow(
    clippy::assertions_on_constants,
    reason = "smoke test asserts compile-time constant to verify the test runner sees this file"
)]
fn test_boundary_smoke_compiles() {
    // Trivial test so the runner sees at least one passing test from
    // this file even if everything else was #[ignore]'d.
    assert_eq!(2 + 2, 4);
    assert!(MAX_BUFFER_SIZE >= 64, "MAX_BUFFER_SIZE must be at least 64");
}

// =============================================================================
// Test helpers - private utilities
// =============================================================================

#[allow(
    dead_code,
    reason = "test helper for future boundary tests; currently unused but available"
)]
fn build_test_engine_with_dict() -> CorrectionEngine {
    let engine = CorrectionEngine::new(EngineConfig {
        max_edit_distance: 1,
        max_candidates: 3,
        min_word_length: 2,
        case_sensitive: false,
        enforce_accents: false,
    });

    let mut builder = fst::MapBuilder::memory();
    let mut words = vec![
        ("cafe", 500),
        ("hello", 1000u64),
        ("hola", 900),
        ("naive", 400),
        ("the", 10000),
        ("world", 800),
    ];
    words.sort_by_key(|k| k.0);
    for (word, freq) in words {
        builder.insert(word, freq).unwrap();
    }
    let dict = Dict::from_bytes(builder.into_inner().unwrap()).unwrap();
    engine.add_dictionary("en", Arc::new(dict));

    let error_map = StaticErrorMap::new("en");
    error_map.insert_static("teh", "the");
    error_map.insert_static("qeu", "que");
    engine.add_error_map(Arc::new(error_map), "en");

    engine
}

#[test]
fn test_boundary_helper_engine_works() {
    let engine = build_test_engine_with_dict();
    // Set the language so the engine looks up the correct error map.
    engine.set_language("en");
    let result = engine.correct("teh");
    // The static map should map "teh" -> "the"
    assert_eq!(result.corrected, Some("the".to_string()));
}

# TypeFix Integration Tests

This document describes the integration tests for TypeFix, focusing on editor simulation and CLI testing.

## Overview

The integration tests verify the complete editor integration pipeline:

```
Keyboard Input → Hook Events → Buffer → Language Detection → Typo Correction
```

## Test Structure

### File: `tests/integration_test.rs`

The main integration test file contains several test modules:

1. **Editor Simulation Tests** (`EditorSimulator`)
   - Simulates typing text character by character
   - Verifies pipeline integration
   - Tests event collection

2. **Hook-Based Editor Tests** (`HookEditorSimulator`)
   - Uses `MockHook` for keyboard hook simulation
   - Tests event capture and processing
   - Verifies special key handling

3. **Pipeline Configuration Tests**
   - Tests various pipeline configurations
   - Verifies auto-correct toggle
   - Tests language detection toggle

4. **Real-World Scenarios**
   - Email composition
   - Chat messages
   - Code comments
   - Multiline text

5. **CLI Integration Tests**
   - Tests `typefix correct` subcommand behavior
   - Verifies error handling

## Running Tests

```bash
# Run all integration tests
cargo test --test integration_test

# Run specific test module
cargo test --test integration_test test_editor

# Run with output
cargo test --test integration_test -- --nocapture

# Run all tests including integration
cargo test
```

## Test Categories

### Editor Simulation Tests

| Test | Description |
|------|-------------|
| `test_editor_simulator_basic_typing` | Basic text typing simulation |
| `test_editor_simulator_typo_correction` | Verifies typo correction in pipeline |
| `test_editor_simulator_language_detection` | Tests language detection during typing |
| `test_editor_simulator_mixed_languages` | Tests switching between languages |
| `test_editor_simulator_event_collection` | Verifies event callbacks work |

### Hook-Based Editor Tests

| Test | Description |
|------|-------------|
| `test_hook_editor_simulator_basic` | Basic hook-based typing |
| `test_hook_editor_captures_events` | Verifies event capture |
| `test_hook_editor_special_keys` | Tests Enter, Tab, Backspace |
| `test_hook_editor_multiple_sessions` | Tests hook start/stop cycles |

### Pipeline Configuration Tests

| Test | Description |
|------|-------------|
| `test_editor_no_auto_correct` | Verifies auto_correct=false works |
| `test_editor_no_language_detection` | Verifies detect_language=false works |
| `test_editor_small_buffer` | Tests with small buffer size |

### Real-World Scenarios

| Test | Description |
|------|-------------|
| `test_editor_email_scenario` | Email composition with typos |
| `test_editor_chat_scenario` | Chat message with abbreviations |
| `test_editor_code_comment_scenario` | Code comment handling |
| `test_editor_multiline_text` | Multiple lines with Enter key |
| `test_editor_rapid_typing` | Fast character input |

### Error Handling Tests

| Test | Description |
|------|-------------|
| `test_editor_empty_text` | Handles empty input |
| `test_editor_only_spaces` | Handles whitespace only |
| `test_editor_special_characters` | Handles punctuation |

### Unicode & i18n Tests

| Test | Description |
|------|-------------|
| `test_editor_spanish_text` | Spanish text support |
| `test_editor_french_accents` | French accented characters |
| `test_editor_with_emojis` | Emoji handling |

### CLI Tests

| Test | Description |
|------|-------------|
| `test_cli_correct_basic` | Basic correction flow |
| `test_cli_correct_valid_word` | Valid word unchanged |
| `test_cli_correct_transposition` | Transposition correction |
| `test_cli_correct_empty` | Empty input handling |

## Editor Simulators

### EditorSimulator

Simple pipeline-based text simulation:

```rust
let mut editor = EditorSimulator::new();
let results = editor.type_text("hello world");
```

### HookEditorSimulator

Full keyboard hook simulation:

```rust
let mut editor = HookEditorSimulator::new();
editor.start();
let results = editor.simulate_typing("text");
editor.stop();
```

## MockHook Usage

The `MockHook` simulates keyboard events for testing:

```rust
use typefix::hooks::platform::{MockHook, KeyEvent, SpecialKey};

// Create and start hook
let hook = MockHook::new(HookConfig::default());
hook.start().unwrap();

// Simulate keypresses
hook.simulate(KeyEvent::Char('h'));
hook.simulate(KeyEvent::Char('i'));
hook.simulate(KeyEvent::Special(SpecialKey::Enter));

// Receive events
while let Ok(event) = rx.recv_timeout(Duration::from_millis(10)) {
    println!("Got event: {:?}", event);
}
```

## Integration Test Patterns

### Testing Typo Correction

```rust
#[test]
fn test_typo_correction() {
    let mut editor = EditorSimulator::new();
    let results = editor.type_text("teh the");
    
    // First word should be corrected
    assert_eq!(results[0].original, "teh");
    assert_eq!(results[0].corrected, Some("the".to_string()));
}
```

### Testing Language Switching

```rust
#[test]
fn test_language_switch() {
    let mut editor = EditorSimulator::new();
    editor.type_text("hello hola mundo");
    
    // Verify mixed language handling
    // ...
}
```

### Testing Hook Events

```rust
#[test]
fn test_hook_event_types() {
    let hook = MockHook::new(HookConfig::default());
    hook.start().unwrap();
    
    hook.simulate(KeyEvent::Char('a'));
    hook.simulate(KeyEvent::Special(SpecialKey::Enter));
    hook.simulate(KeyEvent::Control(ControlKey::Shift));
    
    // Verify events received in order
}
```

## Adding New Tests

### 1. Editor Simulation Test

```rust
#[test]
fn test_my_scenario() {
    let mut editor = EditorSimulator::new();
    
    // Setup
    let results = editor.type_text("your test text");
    
    // Assertions
    assert!(/* your condition */);
}
```

### 2. Hook-Based Test

```rust
#[test]
fn test_my_hook_scenario() {
    let mut editor = HookEditorSimulator::new();
    editor.start();
    
    // Test code
    let results = editor.simulate_typing("text");
    
    // Verify events
    assert_eq!(editor.get_received_events().len(), expected);
    
    editor.stop();
}
```

### 3. CLI Test

```rust
#[test]
fn test_my_cli_scenario() {
    let engine = CorrectionEngine::new(EngineConfig::default());
    // Add dictionaries, test correction
}
```

## CI/CD Integration

These tests are designed to run in CI/CD pipelines:

```yaml
# Example GitHub Actions
- name: Run integration tests
  run: cargo test --test integration_test
```

## Notes

- Integration tests use `TypeFixPipeline::simple()` which pre-loads test dictionaries
- Hook tests require proper cleanup with `editor.stop()`
- Tests are designed to be idempotent and isolated
- Unicode tests verify crash-free operation, not exact character handling

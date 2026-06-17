# TypeFix

**Hyper-lightweight, zero-latency typo correction and language detection engine written in Rust.**

> Open-source project maintained by [Kiroku Solutions](https://github.com/kiroku-solutions).
> Licensed under MIT OR Apache-2.0.

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20or%20Apache--2.0-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Maintained by Kiroku Solutions](https://img.shields.io/badge/maintained%20by-Kiroku%20Solutions-blueviolet.svg)](https://github.com/kiroku-solutions)
[![Crates.io](https://img.shields.io/crates/v/typefix.svg)](https://crates.io/crates/typefix)

## Features

- **Zero-latency correction**: O(1) lookup for known typos, O(m*n) for Damerau-Levenshtein
- **Dynamic language detection**: Bayesian inference with rolling window
- **Memory efficient**: < 10MB RAM footprint (target)
- **Cross-platform**: Windows, Linux, macOS support
- **Fail-safe design**: Graceful degradation on any error
- **No garbage collection**: 100% Rust, zero allocations in hot path

## Performance

Stress test results on typical hardware:

| Benchmark | Result |
|-----------|--------|
| 50K word dictionary insert | 354ms |
| 50K word search | 3.6ms |
| 10K corrections/second | ✅ |
| Memory (idle) | < 10MB target |
| Latency per correction | < 1ms target |

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     TypeFixPipeline                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────┐    ┌──────────────┐    ┌─────────────────────┐   │
│  │CharBuffer│───▶│LanguageDetec│───▶│ CorrectionEngine    │   │
│  │ (64 chars)│    │   tor       │    │  ┌───────────────┐  │   │
│  └──────────┘    │              │    │  │ StaticErrorMap│  │   │
│      │           │  • Bayesian  │    │  │ (O(1) lookup) │  │   │
│      │           │  • Stopwords │    │  └───────────────┘  │   │
│      │           │  • Window     │    │  ┌───────────────┐  │   │
│      │           │    (5 words)  │    │  │ Trie (fuzzy)  │  │   │
│      ▼           └──────────────┘    │  │ (O(m*n) DL)   │  │   │
│  ┌──────────┐                       │  └───────────────┘  │   │
│  │ Delimiter│                       └─────────────────────┘   │
│  │ (space,.)│                                                 │
│  └──────────┘                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Data Flow

1. **Keystroke Input** → `CharBuffer` accumulates characters
2. **Delimiter detected** → Buffer emits word token
3. **Language Detection** → Rolling window of 5 words analyzed
4. **Typo Correction** → Static map (instant) or Damerau-Levenshtein (fuzzy)
5. **Result** → Return corrected word if different

## Quick Start

### Basic Usage

```rust
use typefix::{TypeFixPipeline, PipelineConfig};

// Create simple pipeline with test data
let pipeline = TypeFixPipeline::simple();

// Process text character by character
for ch in "teh world".chars() {
    if let Some(result) = pipeline.push(ch) {
        println!("Word: {}, Corrected: {:?}",
                 result.original,
                 result.corrected);
    }
}
// Output:
// Word: teh, Corrected: Some("the")
// Word: world, Corrected: None
```

### Using Individual Components

```rust
use typefix::{CharBuffer, LanguageDetector, CorrectionEngine};

// Buffer - accumulates keystrokes until delimiter
let buffer = CharBuffer::new();
buffer.push('h');
buffer.push('e');
buffer.push('l');
buffer.push('l');
let word = buffer.push(' '); // Some("hell")

// Language Detection - Bayesian inference
let mut detector = LanguageDetector::new(Default::default());
detector.set_language("en");
detector.process_word("the");

// Correction - static map + fuzzy matching
let mut engine = CorrectionEngine::new(Default::default());
let result = engine.correct("qeu"); // Some("que")
```

### Event Subscription

```rust
use typefix::{TypeFixPipeline, PipelineConfig, PipelineEvent};

let config = PipelineConfig::default();
let mut pipeline = TypeFixPipeline::new(config);

// Subscribe to pipeline events
pipeline.on_event(|event| {
    match event {
        PipelineEvent::WordExtracted { word } => {
            println!("Extracted: {}", word);
        }
        PipelineEvent::WordCorrected { original, corrected } => {
            println!("{} → {}", original, corrected);
        }
        PipelineEvent::LanguageDetected { language, confidence } => {
            println!("Detected: {} ({:.0}%)", language, confidence * 100.0);
        }
    }
});
```

## Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run stress tests
cargo run --bin stress-runner

# Run benchmarks
cargo bench
```

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test file
cargo test --test stress_test

# Run specific test
cargo test test_name
```

## Data Files

```
data/
├── dictionaries/   # Word dictionaries by language
│   ├── es.json     # Spanish
│   ├── en.json     # English
│   └── pt.json     # Portuguese
├── stopwords/      # Stopwords for language detection
│   ├── es.json
│   ├── en.json
│   └── pt.json
└── errors/         # Common typos
    ├── es.json
    └── en.json
```

### Dictionary Format

```json
{
  "language": "es",
  "version": "1.0",
  "words": [
    {"word": "que", "frequency": 1000000},
    {"word": "hola", "frequency": 500000}
  ]
}
```

### Stopwords Format

```json
{
  "language": "es",
  "stopwords": ["el", "la", "de", "que", "y"]
}
```

### Common Errors Format

```json
{
  "language": "es",
  "errors": {
    "qeu": "que",
    "qe": "que",
    "teh": "the"
  }
}
```

## Configuration

### Default Configuration

```rust
// Pipeline
PipelineConfig {
    max_buffer_size: 64,
    enable_correction: true,
    enable_language_detection: true,
}

// Engine
EngineConfig {
    max_edit_distance: 1,
    max_corrections: 3,
    min_word_length: 2,
    dictionary: None,
    error_map: None,
}

// Detector
DetectorConfig {
    window_size: 5,
    confidence_threshold: 0.85,
    hysteresis_zone: 0.10,
    min_words_before_switch: 5,
}
```

### JSON Configuration

```json
{
  "language_detection": {
    "window_size": 5,
    "confidence_threshold": 0.85,
    "hysteresis_zone": 0.10,
    "min_words_before_switch": 5
  },
  "correction": {
    "max_edit_distance": 1,
    "max_corrections": 3,
    "min_word_length": 2
  },
  "buffer": {
    "max_size": 64
  }
}
```

## Project Structure

```
typefix/
├── src/
│   ├── core/           # Core data structures
│   │   ├── buffer.rs   # Ring buffer for keystrokes
│   │   ├── trie.rs     # Trie for dictionary lookups
│   │   └── config.rs   # Configuration management
│   ├── language/       # Language detection
│   │   ├── detector.rs # Bayesian language detector
│   │   └── stopwords.rs# Stopwords trie
│   ├── correction/     # Typo correction
│   │   ├── damerau.rs  # Damerau-Levenshtein distance
│   │   ├── engine.rs   # Correction engine
│   │   └── static_map.rs# Static error map
│   ├── pipeline.rs     # Complete processing pipeline
│   ├── hooks/          # Platform hooks
│   │   ├── platform.rs # Platform abstraction
│   │   ├── windows.rs  # Windows implementation
│   │   ├── linux.rs    # Linux implementation
│   │   └── macos.rs    # macOS implementation
│   ├── memory.rs       # Memory profiling utilities
│   ├── benchmark.rs    # Performance benchmarks
│   └── lib.rs          # Library entry point
├── tests/
│   └── stress_test.rs  # Stress tests and benchmarks
├── docs/
│   ├── plan-implementacion.md
│   ├── final-review-2026-06-16.md
│   └── integration-ehr-legal.md  # EHR/Legal integration guide
├── Cargo.toml
└── README.md
```

## EHR and Legal Integration

The TypeFix supports integration with Electronic Health Records (EHR) and Legal document management systems. See [docs/integration-ehr-legal.md](docs/integration-ehr-legal.md) for:

- **EHR Integration**: HIPAA-compliant configuration, medical terminology support (ICD-10, SNOMED CT), audit trails
- **Legal Integration**: Citation preservation, redlining support, multi-jurisdiction dictionaries
- **API Reference**: Full API documentation with data structures
- **Security Considerations**: PHI handling, audit logging patterns
- **Example Code**: Production-ready integration handlers for both domains

## Implementation Phases

| Phase | Status | Description |
|-------|--------|-------------|
| 1 | ✅ Done | Core engine (Trie, Buffer, Config) |
| 2 | ✅ Done | Language detection |
| 3 | ✅ Done | Correction engine (Damerau-Levenshtein) |
| 4 | ✅ Done | Platform hooks (Windows/Linux/macOS) |
| 5 | ✅ Done | Performance testing (benchmarks, stress tests) |
| 6 | In Progress | Documentation and deployment |
| 7-10 | Pending | Real-world integration testing |

## Contributing

1. Fork the repository
2. Create a feature branch
3. Run tests: `cargo test`
4. Run benchmarks: `cargo run --bin stress-runner`
5. Submit a pull request

## License

TypeFix is open-source software licensed under the **MIT OR Apache-2.0** dual license.

Copyright (c) 2024-2026 Kiroku Solutions. All rights reserved.

See [LICENSE](./LICENSE) and [LICENSE-APACHE](./LICENSE-APACHE) for the full text.

## About Kiroku Solutions

TypeFix is developed and maintained by [Kiroku Solutions](https://github.com/kiroku-solutions), a software consultancy focused on high-performance, low-latency text processing systems. We build open-source tools that make software more accessible to everyone.

- Website: [kiroku.solutions](https://kiroku.solutions)
- GitHub: [@kiroku-solutions](https://github.com/kiroku-solutions)
- Contact: opensource@kiroku.solutions

## Support

- **Issues**: [GitHub Issues](https://github.com/kiroku-solutions/typefix/issues)
- **Discussions**: [GitHub Discussions](https://github.com/kiroku-solutions/typefix/discussions)
- **Security**: See [SECURITY.md](./SECURITY.md) for reporting vulnerabilities

## Contributing

We welcome contributions from the community! See [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines.

Please read our [Code of Conduct](./CODE_OF_CONDUCT.md) before participating.

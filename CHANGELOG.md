# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial open-source release by Kiroku Solutions
- Comprehensive boundary testing (UTF-8, memory, rapid-fire)
- Concurrency testing suite
- CI/CD pipeline with GitHub Actions
- Risk register and security policy
- Multi-agent governance framework

## [0.1.0] - 2026-06-17

### Added
- **Core Engine**:
  - Ring buffer for character input (64 char max)
  - Trie-based dictionary with O(m+n) lookup
  - Damerau-Levenshtein distance with transposition support
  - Static error map for instant correction
  - Configuration loader (JSON, YAML)
- **Language Detection**:
  - Bayesian inference with rolling window
  - Stopwords trie per language
  - Hysteresis to prevent rapid switching
  - Spanish, English, Portuguese support
- **Correction Engine**:
  - Static map priority (O(1) lookup)
  - Dictionary fallback with fuzzy matching
  - Case preservation
  - Punctuation preservation
  - User-learned corrections
- **Pipeline**:
  - TypeFixPipeline orchestrator
  - Event emission (WordExtracted, LanguageDetected, WordCorrected)
  - Configurable auto-correct
  - Buffer overflow protection
- **Platform Hooks**:
  - Windows low-level keyboard hook (WH_KEYBOARD_LL)
  - Linux XCB support
  - macOS Core Graphics support
  - Mock hook for testing
- **Tooling**:
  - CLI binary (`repl`, `correct`, `bench` subcommands)
  - Memory profiling utilities
  - Performance benchmarks
  - Stress test suite
- **Documentation**:
  - Architecture overview
  - EHR/Legal integration guide
  - Cross-compilation instructions
  - 10-phase implementation plan

### Security
- 100% safe Rust in production code
- Zero unsafe blocks in hot paths
- No network I/O
- No file system writes by default
- No telemetry or analytics
- Memory-safe with controlled buffer overflow
- Fail-silent design: errors never crash host process

### Performance
- <1ms per correction target
- <10MB RAM footprint target
- Zero allocations in hot path
- Multi-threaded stress test passing

## Versioning

This project uses [Semantic Versioning](https://semver.org/):
- **MAJOR**: incompatible API changes
- **MINOR**: backward-compatible features
- **PATCH**: backward-compatible bug fixes

## Release Notes

Releases are tracked on [GitHub](https://github.com/kiroku-solutions/typefix/releases).
See [SECURITY.md](./SECURITY.md) for security update policy.

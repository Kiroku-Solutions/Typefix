# Contributing to TypeFix

Thank you for your interest in contributing to TypeFix! This project is maintained by [Kiroku Solutions](https://github.com/kiroku-solutions) and the open-source community. Every contribution, from a typo fix to a major feature, is welcome.

## Code of Conduct

This project adheres to a [Code of Conduct](./CODE_OF_CONDUCT.md). By participating, you are expected to uphold it. Please report unacceptable behavior to opensource@kiroku.solutions.

## How to Contribute

### Reporting Bugs

- Check the [issue tracker](https://github.com/kiroku-solutions/typefix/issues) to avoid duplicates.
- Use the bug report template.
- Include Rust version (`rustc --version`), OS, and a minimal reproduction.

### Suggesting Features

- Open an issue with the "enhancement" label.
- Describe the use case and proposed API.

### Pull Requests

1. Fork the repository.
2. Create a feature branch: `git checkout -b feature/my-change`
3. Make your changes following the project conventions.
4. Add tests for new behavior.
5. Run the full test suite: `cargo test`
6. Run clippy: `cargo clippy --all-targets -- -D warnings`
7. Run formatter: `cargo fmt`
8. Commit with a descriptive message.
9. Push and open a pull request.

## Development Setup

### Requirements

- Rust 1.70+ (stable)
- Windows: Visual Studio Build Tools
- Linux: `build-essential` and `libxkbcommon-dev`
- macOS: Xcode Command Line Tools
- Optional: `just` ([install](https://github.com/casey/just)) for ergonomic task running
- Optional: `cargo-llvm-cov` for coverage (`cargo install cargo-llvm-cov`)

### Building

```bash
cargo build
cargo test
cargo bench
```

### Local CI (recommended)

Before every push, run the same checks GitHub Actions runs. This is the single most effective way to avoid broken commits and review churn.

**Using the script directly:**

```bash
# Windows PowerShell
.\scripts\ci-local.ps1

# Linux / macOS
./scripts/ci-local.sh

# Skip slow checks (release build, stress tests, coverage)
./scripts/ci-local.sh --quick          # bash
.\scripts\ci-local.ps1 -Quick          # PowerShell
```

**Using `just`** (requires [just](https://github.com/casey/just)):

```bash
just ci          # full local CI
just ci-quick    # quick mode
just fmt         # format code
just clippy      # run clippy
just test        # run tests
just bench       # run benchmarks
just coverage    # generate HTML coverage
just doctor      # print versions of installed tools
```

### Git pre-push hook

Install the pre-push hook to run the local CI suite automatically before every `git push`. Pushes that fail CI are blocked; bypass with `git push --no-verify` only when you have a reason.

```bash
# Windows PowerShell
.\scripts\install-hooks.ps1

# Linux / macOS
./scripts/install-hooks.sh
```

After installation, `.githooks/` is used as the hooksPath so hooks stay version-controlled and the install is reproducible across machines.

### Why this exists

The CI pipeline runs six jobs (fmt, clippy, build, test, coverage, committee-rules). If a single one fails, the entire push is rejected by the gate. Running the same checks locally catches 95% of issues before the round-trip to GitHub, so you only push when the build is green.


### Project Structure

```
typefix/
├── src/
│   ├── core/         # Buffer, Trie, Config
│   ├── language/     # Language detector and stopwords
│   ├── correction/   # Damerau-Levenshtein, error map, engine
│   ├── hooks/        # Platform-specific keyboard hooks
│   ├── pipeline.rs   # Top-level orchestration
│   ├── memory.rs     # Memory profiling utilities
│   └── benchmark.rs  # Performance benchmarks
├── tests/            # Integration and stress tests
├── docs/             # Design docs and guides
└── benches/          # Criterion benchmarks
```

## Coding Standards

- **Style**: `cargo fmt` (no exceptions)
- **Lints**: `cargo clippy --all-targets -- -D warnings` must pass
- **Tests**: New behavior must have tests
- **Documentation**: Public APIs require `///` rustdoc comments
- **No `unwrap()`** in production code paths; use `?` and `anyhow::Result`
- **No panics** in the hot path; use graceful degradation
- **Error messages**: descriptive, include context
- **Commit messages**: imperative mood, reference issues when applicable

## Architecture Guidelines

- **No global state**: All concurrency uses `Arc<RwLock<T>>` (see `docs/sec-7-analysis.md`)
- **Zero-cost abstractions**: prefer compile-time dispatch
- **Fail-silent design**: errors degrade gracefully, never crash the host process
- **Memory ceiling**: target <10MB RAM, no allocations in hot path
- **Latency ceiling**: target <1ms per correction

## Pull Request Checklist

- [ ] Tests pass locally (`cargo test`)
- [ ] Clippy is clean (`cargo clippy --all-targets -- -D warnings`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Public APIs have rustdoc
- [ ] CHANGELOG.md updated
- [ ] No new unsafe code (or justified)
- [ ] PR description links the related issue

## Release Process

Releases are managed by Kiroku Solutions maintainers. Versioning follows [SemVer](https://semver.org/). Tagged releases trigger automatic publishing to crates.io via CI.

## License

By contributing, you agree that your contributions will be licensed under the same MIT OR Apache-2.0 dual license as the project.

## Contact

- General questions: GitHub Discussions
- Security issues: see [SECURITY.md](./SECURITY.md)
- Direct contact: opensource@kiroku.solutions

# TypeFix local development tasks
# Install `just` from https://github.com/casey/just
# Run `just` to see the list of available recipes.

set shell := ["powershell", "-NoProfile", "-Command"]

# Default recipe: show available commands
default:
    @just --list

# Format the code
fmt:
    cargo fmt --all

# Check formatting (CI parity)
fmt-check:
    cargo fmt --all -- --check

# Run clippy with warnings as errors
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Build the project (debug)
build:
    cargo build --all-targets

# Build optimized release binary
build-release:
    cargo build --release

# Run all tests
test:
    cargo test --all-features

# Run integration tests
test-integration:
    cargo test --test integration_test --test boundary_test --test concurrency_test

# Run stress tests
test-stress:
    cargo test --release --test stress_test -- --nocapture

# Run benchmarks
bench:
    cargo bench

# Run the full local CI suite (matches .github/workflows/ci.yml)
ci:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/ci-local.ps1

# Run the quick local CI suite (skip release build, stress, coverage)
ci-quick:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/ci-local.ps1 -Quick

# Generate coverage report
coverage:
    cargo llvm-cov --all-features --workspace --html

# Watch for changes and rebuild
watch:
    cargo watch -x check -x test

# Clean build artifacts
clean:
    cargo clean

# Update all dependencies
update:
    cargo update

# Print versions
doctor:
    rustc --version
    cargo --version
    @if (Get-Command cargo-llvm-cov -ErrorAction SilentlyContinue) { cargo-llvm-cov --version } else { Write-Host "cargo-llvm-cov: not installed" }
    @if (Get-Command cargo-watch -ErrorAction SilentlyContinue) { cargo-watch --version } else { Write-Host "cargo-watch: not installed" }

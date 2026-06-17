# Cross-Compilation Guide for TypeFix

This document describes how to cross-compile the TypeFix for different platforms.

## Supported Targets

| Target | Platform | Description |
|--------|----------|-------------|
| `x86_64-unknown-linux-musl` | Linux (x86_64) | Static binaries, no libc dependencies |
| `x86_64-apple-darwin` | macOS (Intel) | Native Intel Mac binaries |
| `aarch64-apple-darwin` | macOS (ARM64) | Apple Silicon (M1/M2/M3) binaries |

## Configuration

Cross-compilation settings are in `.cargo/config.toml`:

```toml
[target.x86_64-unknown-linux-musl]
rustflags = [
    "-C", "target-feature=+crt-static",
    "-C", "link-arg=-static",
    "-C", "link-arg=-pthread",
]

[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-mmacosx-version-min=10.15"]

[target.aarch64-apple-darwin]
rustflags = ["-C", "link-arg=-mmacosx-version-min=10.15"]
```

## Prerequisites

### Install Rust Targets

```bash
rustup target add x86_64-unknown-linux-musl
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin
```

### Install Cross-Compilation Tools

#### For Linux musl (from macOS/Linux)

```bash
# Using Homebrew
brew install FiloSottile/musl-cross/musl-cross

# Or using cross (recommended)
cargo install cross
```

## Building

### Native Build

```bash
cargo build --release
```

### Linux musl (static binary)

**Option 1: Using cross (recommended)**

```bash
cross build --target x86_64-unknown-linux-musl --release
```

**Option 2: Direct build**

```bash
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc
cargo build --target x86_64-unknown-linux-musl --release
```

### macOS Intel

```bash
cargo build --target x86_64-apple-darwin --release
```

### macOS ARM64 (Apple Silicon)

```bash
cargo build --target aarch64-apple-darwin --release
```

## Universal macOS Binary

Create a fat binary that works on both Intel and Apple Silicon:

```bash
cargo build --target x86_64-apple-darwin --release
cargo build --target aarch64-apple-darwin --release

lipo -create \
    target/x86_64-apple-darwin/release/typefix \
    target/aarch64-apple-darwin/release/typefix \
    -output target/universal-apple-darwin/release/typefix
```

## GitHub Actions Workflow

```yaml
name: Release

on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        target:
          - x86_64-unknown-linux-musl
          - x86_64-apple-darwin
          - aarch64-apple-darwin
    runs-on: ${{ matrix.target == 'aarch64-apple-darwin' && 'macos-latest' || 'ubuntu-latest' }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - name: Build
        run: cargo build --release --target ${{ matrix.target }}
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: typefix-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/typefix
```

## Troubleshooting

### Linux: "error: linking with `cc` failed"

Install musl tools:
```bash
# Debian/Ubuntu
sudo apt install musl-dev

# macOS
brew install FiloSottile/musl-cross/musl-cross
```

### macOS: "error: cannot find framework"

Cross-compiling to macOS from Windows/Linux requires macOS SDK. Use GitHub Actions for macOS builds.

### Windows: Cannot cross-compile to macOS

Use GitHub Actions with macOS runners or a macOS VM.

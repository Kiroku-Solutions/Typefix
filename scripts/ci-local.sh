#!/usr/bin/env bash
#
# Runs the same checks as the GitHub Actions CI pipeline, locally.
# Mirrors .github/workflows/ci.yml so contributors can catch issues
# before pushing. Exits non-zero on any failure.
#
# Usage:
#   ./scripts/ci-local.sh           # full suite
#   ./scripts/ci-local.sh --quick   # skip release build, stress tests, coverage
#   ./scripts/ci-local.sh --no-cov  # skip coverage

set -euo pipefail
FAILED=0
QUICK=0
NO_COV=0

for arg in "$@"; do
    case "$arg" in
        --quick) QUICK=1 ;;
        --no-cov) NO_COV=1 ;;
        *) echo "Unknown arg: $arg" >&2; exit 2 ;;
    esac
done

step() {
    local name="$1"; shift
    echo
    echo "==> $name"
    if "$@"; then
        echo "    OK: $name"
    else
        echo "    FAILED: $name" >&2
        FAILED=1
    fi
}

# 1. Format
step "Format (cargo fmt --check)" cargo fmt --all -- --check

# 2. Clippy
step "Clippy (cargo clippy -D warnings)" \
    cargo clippy --all-targets --all-features -- -D warnings

# 3. Build (debug)
step "Build (debug)" cargo build --all-targets

# 4. Build (release)
if [ "$QUICK" -eq 0 ]; then
    step "Build (release)" cargo build --release
fi

# 5. Tests
step "Tests (cargo test)" cargo test --all-features

# 6. Stress tests
if [ "$QUICK" -eq 0 ]; then
    step "Stress tests (release)" \
        cargo test --release --test stress_test -- --nocapture
fi

# 7. Coverage
if [ "$NO_COV" -eq 0 ]; then
    if command -v cargo-llvm-cov >/dev/null 2>&1; then
        echo
        echo "==> Coverage (cargo llvm-cov, 90% gate)"
        if cargo llvm-cov --all-features --workspace --summary-only --json \
            > coverage.json 2>/dev/null; then
            PCT=$(python3 -c "import json; print(json.load(open('coverage.json'))['data']['summary']['lines']['percent'])")
            printf "    Line coverage: %.2f%%\n" "$PCT"
            if [ "$(echo "$PCT < 90.0" | bc -l)" = "1" ]; then
                echo "    FAILED: coverage below 90% gate" >&2
                FAILED=1
            else
                echo "    OK: coverage gate passed"
            fi
            rm -f coverage.json
        else
            echo "    cargo llvm-cov failed" >&2
            FAILED=1
        fi
    else
        echo
        echo "==> Coverage skipped (cargo-llvm-cov not installed)"
    fi
fi

# 8. Committee rules
echo
echo "==> Committee rules (no unwrap / FIXME / TODO in src/)"
UNWRAPS=$(grep -RInE "\.(unwrap|expect)[[:space:]]*\(" src/ || true)
FIXMES=$(grep -RInE "\b(FIXME|TODO)\b" src/ || true)
COMMITTEE_OK=1
if [ -n "$UNWRAPS" ]; then
    echo "    FAILED: unwrap/expect found in production code:" >&2
    echo "$UNWRAPS" >&2
    COMMITTEE_OK=0
fi
if [ -n "$FIXMES" ]; then
    echo "    FAILED: FIXME/TODO found in production code:" >&2
    echo "$FIXMES" >&2
    COMMITTEE_OK=0
fi
if [ "$COMMITTEE_OK" -eq 1 ]; then
    echo "    OK: no unwrap / FIXME / TODO in src/"
else
    FAILED=1
fi

echo
if [ "$FAILED" -ne 0 ]; then
    echo "CI FAILED. Fix the issues above before pushing." >&2
    exit 1
fi
echo "ALL CHECKS PASSED. Safe to push."

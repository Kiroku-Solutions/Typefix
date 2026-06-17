<#
.SYNOPSIS
    Runs the same checks as the GitHub Actions CI pipeline, locally.
#>

[CmdletBinding()]
param(
    [switch]$Quick,
    [switch]$SkipCoverage
)

$ErrorActionPreference = "Stop"
$failed = $false

function Step {
    param([string]$Name, [scriptblock]$Block)
    Write-Host ""
    Write-Host "==> $Name" -ForegroundColor Cyan
    & $Block
    if ($LASTEXITCODE -ne 0) {
        $script:failed = $true
        Write-Host "    FAILED: $Name" -ForegroundColor Red
    } else {
        Write-Host "    OK: $Name" -ForegroundColor Green
    }
}

# 1. Format check
Step "Format (cargo fmt --check)" { cargo fmt --all -- --check }

# 2. Clippy
Step "Clippy (cargo clippy -D warnings)" { cargo clippy --all-targets --all-features -- -D warnings }

# 3. Build (debug)
Step "Build (debug)" { cargo build --all-targets }

if (-not $Quick) { Step "Build (release)" { cargo build --release } }

# 5. Tests
Step "Tests (cargo test)" { cargo test --all-features }

if (-not $Quick) { Step "Stress tests (release)" { cargo test --release --test stress_test -- --nocapture } }

# 7. Coverage
if (-not $SkipCoverage) {
    Step "Coverage (cargo llvm-cov, 90% gate)" {
        if (-not (Get-Command cargo-llvm-cov -ErrorAction SilentlyContinue)) {
            Write-Host "    cargo-llvm-cov not installed; skipping" -ForegroundColor Yellow; return
        }
        cargo llvm-cov --all-features --workspace --summary-only --json | Tee-Object -FilePath coverage.json | Out-Null
        $data = Get-Content coverage.json -Raw | ConvertFrom-Json
        $pct = $data.data.summary.lines.percent
        Write-Host ("    Line coverage: {0:N2}%%" -f $pct) -ForegroundColor Cyan
        if ($pct -lt 90.0) { Write-Host "    FAILED: coverage below 90% gate" -ForegroundColor Red; $script:failed = $true }
        else { Write-Host "    OK: coverage gate passed" -ForegroundColor Green }
        Remove-Item coverage.json -ErrorAction SilentlyContinue
    }
}

# 8. Committee rules
# Excludes:
#   - clippy attribute declarations (e.g. #![deny(clippy::todo)])
#   - lines inside #[cfg(test)] mod tests { ... } blocks
#   - lines inside #[test] fn name() { ... } blocks
Step "Committee rules (no unwrap / FIXME / TODO in production code)" {
    $srcFiles = Get-ChildItem -Path src -Recurse -Include *.rs -File
    $unwrapHits = @()
    $fixmeHits = @()

    foreach ($f in $srcFiles) {
        $lines = Get-Content $f.FullName -Encoding UTF8
        $inCfgTest = $false
        $cfgTestDepth = 0
        $inTestFn = $false
        $testFnBraceDepth = 0
        $overallBraceDepth = 0

        $justSetCfgTest = $false
        $justSetTestFn = $false

        for ($i = 0; $i -lt $lines.Count; $i++) {
            $line = $lines[$i]
            $trim = $line.Trim()
            $opens = ([regex]::Matches($line, '\{')).Count
            $closes = ([regex]::Matches($line, '\}')).Count

            $isClippyAttr = $trim -match '^\s*clippy::' -or $trim -match '^#!?\s*\[.*clippy::'

            if ($trim -match '^#!?\s*\[cfg\(test\)\]') {
                $inCfgTest = $true
                $cfgTestDepth = $overallBraceDepth
                $justSetCfgTest = $true
            }
            if ($trim -match '^#!?\s*\[test\s*(\(.*\))?\s*\]') {
                $inTestFn = $true
                $testFnBraceDepth = $overallBraceDepth
                $justSetTestFn = $true
            }

            $isProduction = -not ($inCfgTest -or $inTestFn)

            if ($isProduction -and -not $isClippyAttr) {
                if ($trim -match '\.(unwrap|expect)\s*\(') {
                    $unwrapHits += [pscustomobject]@{ Filename = $f.FullName; LineNumber = $i + 1; Line = $line }
                }
                if ($trim -match '\b(FIXME|TODO)\b') {
                    $fixmeHits += [pscustomobject]@{ Filename = $f.FullName; LineNumber = $i + 1; Line = $line }
                }
            }

            $overallBraceDepth += $opens - $closes
            if ($inCfgTest -and -not $justSetCfgTest -and $overallBraceDepth -le $cfgTestDepth) { $inCfgTest = $false }
            if ($inTestFn -and -not $justSetTestFn -and $overallBraceDepth -le $testFnBraceDepth) { $inTestFn = $false }

            $justSetCfgTest = $false
            $justSetTestFn = $false
        }
    }

    $localFailed = $false
    if ($unwrapHits) {
        Write-Host "    FAILED: unwrap/expect found in production code:" -ForegroundColor Red
        $unwrapHits | ForEach-Object { Write-Host ("      {0}:{1}: {2}" -f $_.Filename, $_.LineNumber, $_.Line.Trim()) -ForegroundColor Red }
        $localFailed = $true
    }
    if ($fixmeHits) {
        Write-Host "    FAILED: FIXME/TODO found in production code:" -ForegroundColor Red
        $fixmeHits | ForEach-Object { Write-Host ("      {0}:{1}: {2}" -f $_.Filename, $_.LineNumber, $_.Line.Trim()) -ForegroundColor Red }
        $localFailed = $true
    }
    if (-not $localFailed) {
        Write-Host "    OK: no unwrap / FIXME / TODO in production code" -ForegroundColor Green
        exit 0
    } else {
        exit 1
    }
}

Write-Host ""
if ($failed) {
    Write-Host "CI FAILED. Fix the issues above before pushing." -ForegroundColor Red
    exit 1
} else {
    Write-Host "ALL CHECKS PASSED. Safe to push." -ForegroundColor Green
    exit 0
}

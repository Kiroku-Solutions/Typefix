<#
.SYNOPSIS
    Installs the git hooks from .githooks/ into .git/hooks/.

.DESCRIPTION
    Configures git to use .githooks/ as the hooksPath so that hooks
    stay version-controlled and the install is reproducible.

.EXAMPLE
    .\scripts\install-hooks.ps1
#>

$ErrorActionPreference = "Stop"

$repoRoot = git rev-parse --show-toplevel
$hooksSrc = Join-Path $repoRoot ".githooks"
$hooksDst = Join-Path $repoRoot ".git/hooks"

if (-not (Test-Path $hooksSrc)) {
    Write-Error "No .githooks directory found at $hooksSrc"
    exit 1
}
if (-not (Test-Path $hooksDst)) {
    Write-Error "No .git/hooks directory found at $hooksDst. Is this a git repository?"
    exit 1
}

# Configure git to use .githooks/ as the hooksPath.
git config core.hooksPath .githooks
Write-Host "Configured git to use .githooks/ as the hooks directory."

# Make sure all hook scripts are executable.
Get-ChildItem -Path $hooksSrc -File | ForEach-Object {
    & git update-index --chmod=+x $_.FullName 2>$null | Out-Null
}

Write-Host "Hooks installed:"
Get-ChildItem $hooksSrc | ForEach-Object { Write-Host "  $($_.Name)" }

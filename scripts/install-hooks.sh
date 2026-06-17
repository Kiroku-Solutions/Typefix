#!/usr/bin/env bash
# Installs the git hooks from .githooks/ into .git/hooks/.
# Run this once after cloning the repository.

set -e

REPO_ROOT="$(git rev-parse --show-toplevel)"
HOOKS_SRC="$REPO_ROOT/.githooks"
HOOKS_DST="$REPO_ROOT/.git/hooks"

if [ ! -d "$HOOKS_SRC" ]; then
    echo "No .githooks directory found at $HOOKS_SRC" >&2
    exit 1
fi

if [ ! -d "$HOOKS_DST" ]; then
    echo "No .git/hooks directory found at $HOOKS_DST" >&2
    echo "Is this a git repository?" >&2
    exit 1
fi

# Configure git to use .githooks/ as the hooksPath.
# This avoids copying files and keeps the hooks version-controlled.
git config core.hooksPath .githooks
echo "Configured git to use .githooks/ as the hooks directory."

# Make sure all hook scripts are executable.
find "$HOOKS_SRC" -type f -exec chmod +x {} \;

echo "Hooks installed:"
ls -1 "$HOOKS_SRC"

#!/usr/bin/env bash
# Install mnemosyne git hooks from versioned source (scripts/hooks/) to
# .git/hooks/. Idempotent — overwrites existing hooks.
#
# Round 79 OPTION C carry — Phase 0 self-application carry.

set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
src="$repo_root/scripts/hooks"
dst="$repo_root/.git/hooks"

for hook in "$src"/*; do
 name=$(basename "$hook")
 cp "$hook" "$dst/$name"
 chmod +x "$dst/$name"
 echo "installed: $dst/$name"
done

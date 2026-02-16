#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

"$REPO_ROOT/scripts/clean-heavy.sh"

echo "Cleaning full local reproducible caches in: $REPO_ROOT"

for path in \
  "$REPO_ROOT/frontend/node_modules" \
  "$REPO_ROOT/.codex_audit"
do
  if [ -d "$path" ]; then
    rm -rf "$path"
    echo "Removed: $path"
  fi
done

find "$REPO_ROOT" -name ".DS_Store" -type f -delete

echo "Full local cleanup complete."

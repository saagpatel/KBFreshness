#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "Cleaning heavy build artifacts in: $REPO_ROOT"

for path in \
  "$REPO_ROOT/target" \
  "$REPO_ROOT/frontend/dist" \
  "$REPO_ROOT/frontend/.vite" \
  "$REPO_ROOT/.lean-dev-cache"
do
  if [ -d "$path" ]; then
    rm -rf "$path"
    echo "Removed: $path"
  fi
done

echo "Heavy artifact cleanup complete."

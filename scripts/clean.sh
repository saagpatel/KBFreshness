#!/usr/bin/env bash
set -euo pipefail

# Clean local build/install artifacts without touching source files.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "Cleaning local artifacts in: $REPO_ROOT"

find "$REPO_ROOT" -name ".DS_Store" -type f -delete

if [ -d "$REPO_ROOT/target" ]; then
  find "$REPO_ROOT/target" -mindepth 1 -delete
  rmdir "$REPO_ROOT/target" 2>/dev/null || true
fi

if [ -d "$REPO_ROOT/frontend/node_modules" ]; then
  find "$REPO_ROOT/frontend/node_modules" -mindepth 1 -delete
  rmdir "$REPO_ROOT/frontend/node_modules" 2>/dev/null || true
fi

if [ -d "$REPO_ROOT/frontend/dist" ]; then
  find "$REPO_ROOT/frontend/dist" -mindepth 1 -delete
  rmdir "$REPO_ROOT/frontend/dist" 2>/dev/null || true
fi

if [ -d "$REPO_ROOT/.codex_audit" ]; then
  find "$REPO_ROOT/.codex_audit" -mindepth 1 -delete
  rmdir "$REPO_ROOT/.codex_audit" 2>/dev/null || true
fi

echo "Cleanup complete."

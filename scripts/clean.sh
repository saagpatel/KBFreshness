#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Backward-compatible wrapper for full local cleanup.
"$REPO_ROOT/scripts/clean-all.sh"

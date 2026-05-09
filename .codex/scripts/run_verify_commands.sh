#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERIFY_FILE="$ROOT_DIR/.codex/verify.commands"

cd "$ROOT_DIR"

if [[ ! -f "$VERIFY_FILE" ]]; then
  echo "Missing verify commands file: $VERIFY_FILE" >&2
  exit 1
fi

while IFS= read -r command || [[ -n "$command" ]]; do
  if [[ -z "$command" || "$command" =~ ^# ]]; then
    continue
  fi
  echo "[verify] $command"
  bash -lc "$command"
done < "$VERIFY_FILE"

echo "[verify] all commands passed"

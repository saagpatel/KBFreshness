#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEAN_ROOT="$REPO_ROOT/.lean-dev-cache"
BACKEND_CACHE_DIR="$LEAN_ROOT/cargo-target"
FRONTEND_CACHE_DIR="$LEAN_ROOT/vite-cache"

mkdir -p "$BACKEND_CACHE_DIR" "$FRONTEND_CACHE_DIR"

BACKEND_PID=""
FRONTEND_PID=""

cleanup() {
  set +e
  echo ""
  echo "Shutting down lean dev services..."
  if [ -n "$FRONTEND_PID" ] && kill -0 "$FRONTEND_PID" 2>/dev/null; then
    kill "$FRONTEND_PID" 2>/dev/null || true
    wait "$FRONTEND_PID" 2>/dev/null || true
  fi
  if [ -n "$BACKEND_PID" ] && kill -0 "$BACKEND_PID" 2>/dev/null; then
    kill "$BACKEND_PID" 2>/dev/null || true
    wait "$BACKEND_PID" 2>/dev/null || true
  fi
  rm -rf "$LEAN_ROOT"
  echo "Removed lean cache directory: $LEAN_ROOT"
}

trap cleanup EXIT INT TERM

echo "Starting backend (lean cache at $BACKEND_CACHE_DIR)..."
(
  cd "$REPO_ROOT"
  CARGO_TARGET_DIR="$BACKEND_CACHE_DIR" cargo run --features "screenshots,tickets"
) &
BACKEND_PID=$!

echo "Starting frontend (lean cache at $FRONTEND_CACHE_DIR)..."
(
  cd "$REPO_ROOT/frontend"
  VITE_CACHE_DIR="$FRONTEND_CACHE_DIR" npm run dev
) &
FRONTEND_PID=$!

echo "Lean dev mode running."
echo "Backend:  http://localhost:3001"
echo "Frontend: http://localhost:5173"
echo "Press Ctrl+C to stop and clean lean build caches."

wait "$BACKEND_PID" "$FRONTEND_PID"

#!/usr/bin/env bash
set -euo pipefail

# codex-os-managed
max_bytes="${ASSET_MAX_BYTES:-350000}"
asset_roots=()
[[ -d dist/assets ]] && asset_roots+=("dist/assets")
[[ -d public ]] && asset_roots+=("public")

if [[ ${#asset_roots[@]} -eq 0 ]]; then
  echo "No built asset directory found; skipping asset check."
  mkdir -p .perf-results
  printf '{\n  "status": "not-run",\n  "reason": "no_asset_directory"\n}\n' > .perf-results/assets.json
  exit 0
fi

fail=0
largest_file=""
largest_size=0
while IFS= read -r file; do
  size=$(wc -c < "$file")
  if (( size > largest_size )); then
    largest_size=$size
    largest_file="$file"
  fi
  if (( size > max_bytes )); then
    echo "Asset too large (>${max_bytes} bytes): $file"
    fail=1
  fi
done < <(find "${asset_roots[@]}" -type f \( -name "*.png" -o -name "*.jpg" -o -name "*.jpeg" -o -name "*.webp" -o -name "*.avif" -o -name "*.js" -o -name "*.css" \))

mkdir -p .perf-results
ASSET_FAIL="$fail" ASSET_MAX_BYTES="$max_bytes" ASSET_LARGEST_FILE="$largest_file" ASSET_LARGEST_SIZE="$largest_size" ASSET_ROOTS="$(printf '%s\n' "${asset_roots[@]}")" python3 - <<'PY'
import json
import os
from pathlib import Path

path = Path(".perf-results/assets.json")
status = "fail" if os.environ["ASSET_FAIL"] == "1" else "pass"
checked_roots = [line for line in os.environ["ASSET_ROOTS"].splitlines() if line]
payload = {
    "status": status,
    "maxBytes": int(os.environ["ASSET_MAX_BYTES"]),
    "largestFile": os.environ["ASSET_LARGEST_FILE"],
    "largestBytes": int(os.environ["ASSET_LARGEST_SIZE"]),
    "checkedRoots": checked_roots,
}
path.write_text(json.dumps(payload, indent=2) + "\n")
PY

exit $fail

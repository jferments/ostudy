#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

while IFS= read -r -d '' script; do
  bash -n "$script"
done < <(
  find "$ROOT_DIR" \
    -path "$ROOT_DIR/.git" -prune -o \
    -path "$ROOT_DIR/.artifacts" -prune -o \
    -path "$ROOT_DIR/node_modules" -prune -o \
    -path "$ROOT_DIR/src-tauri/target" -prune -o \
    -type f -name '*.sh' -print0
)

while IFS= read -r -d '' script; do
  node --check "$script" >/dev/null
done < <(
  find "$ROOT_DIR" \
    -path "$ROOT_DIR/.git" -prune -o \
    -path "$ROOT_DIR/.artifacts" -prune -o \
    -path "$ROOT_DIR/node_modules" -prune -o \
    -path "$ROOT_DIR/src-tauri/target" -prune -o \
    -type f \( -name '*.mjs' -o -name '*.cjs' \) -print0
)

python3 - "$ROOT_DIR" <<'PY'
from __future__ import annotations

import ast
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
excluded = {".artifacts", ".git", "node_modules", "target", "gen", "dist"}
checked = 0
for path in root.rglob("*.py"):
    if any(part in excluded for part in path.parts):
        continue
    source = path.read_text(encoding="utf-8")
    ast.parse(source, filename=str(path))
    checked += 1

if checked == 0:
    raise SystemExit("ERROR: no Python scripts were found for syntax validation.")
print(f"OK   Python syntax: {checked} file(s)")
PY

printf 'OK   Bash and Node script syntax\n'

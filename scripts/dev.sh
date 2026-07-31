#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
cd "$ROOT_DIR"
exec node "$SCRIPT_DIR/with-immutable-cargo-lock.mjs" pnpm tauri dev -- --locked

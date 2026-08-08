#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

node ./scripts/sync-migration-checksums.mjs "$@"
cargo metadata --manifest-path src-tauri/Cargo.toml --format-version 1 >/dev/null
cargo fmt --manifest-path src-tauri/Cargo.toml --all

echo "Canonical migration checksums, Cargo.lock, and Rust formatting are synchronized."

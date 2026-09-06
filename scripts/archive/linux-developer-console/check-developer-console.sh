#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
TOOL_DIR="$SCRIPT_DIR/developer-console"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"

grep -Fq 'ratatui = "='"$RATATUI_VERSION"'"' "$TOOL_DIR/Cargo.toml"
grep -Fq 'crossterm = "='"$CROSSTERM_VERSION"'"' "$TOOL_DIR/Cargo.toml"

TMP_DIR="$(mktemp -d -t ostudy-developer-console.XXXXXXXX)"
cleanup() { rm -rf -- "$TMP_DIR"; }
trap cleanup EXIT

mkdir -p "$TMP_DIR/src"
cp "$TOOL_DIR/Cargo.toml" "$TMP_DIR/Cargo.toml"
cp "$TOOL_DIR"/src/*.rs "$TMP_DIR/src/"

# Resolve/build in a temporary source copy so Cargo.lock generation can never
# mutate the maintained repository during qualification. Direct TUI deps are
# exact-pinned; the generated transient lock is still exercised by this run.
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$TMP_DIR/target}" \
  cargo test --manifest-path "$TMP_DIR/Cargo.toml"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$TMP_DIR/target}" \
  cargo clippy --manifest-path "$TMP_DIR/Cargo.toml" --all-targets -- -D warnings

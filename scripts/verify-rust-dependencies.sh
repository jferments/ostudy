#!/usr/bin/env bash
set -Eeuo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
metadata="$(cargo metadata --manifest-path "$ROOT_DIR/src-tauri/Cargo.toml" --locked --format-version 1)"
check_crate() {
  local crate="$1" expected="$2" actual
  actual="$(jq -r --arg name "$crate" '[.packages[] | select(.name == $name) | .version] | unique | join(",")' <<<"$metadata")"
  [[ "$actual" == "$expected" ]] || { printf 'ERROR: %s expected %s, resolved %s\n' "$crate" "$expected" "$actual" >&2; exit 1; }
  printf 'OK   %s@%s\n' "$crate" "$actual"
}
check_absent() {
  local crate="$1" count
  count="$(jq -r --arg name "$crate" '[.packages[] | select(.name == $name)] | length' <<<"$metadata")"
  [[ "$count" == "0" ]] || { printf 'ERROR: prohibited crate %s is present in the resolved graph\n' "$crate" >&2; exit 1; }
  printf 'OK   %s absent\n' "$crate"
}
check_crate tauri "$TAURI_RUST_VERSION"
check_crate tauri-build "$TAURI_BUILD_VERSION"
check_crate rusqlite "$RUSQLITE_VERSION"
check_crate serde "$SERDE_VERSION"
check_crate serde_json "$SERDE_JSON_VERSION"
check_crate tauri-plugin-dialog "$TAURI_DIALOG_VERSION"
check_crate tauri-plugin-opener "$TAURI_OPENER_VERSION"
check_crate chrono "$CHRONO_VERSION"
check_crate image "$IMAGE_VERSION"
check_crate oxidize-pdf "$OXIDIZE_PDF_VERSION"
check_crate pdfium-render "$PDFIUM_RENDER_VERSION"
check_crate rs-fsrs "$RS_FSRS_VERSION"
check_absent justpdf
check_absent rusty-tesseract

#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$ROOT_DIR/scripts/cargo-target.sh"
# shellcheck disable=SC1091
source "$ROOT_DIR/scripts/build-state.sh"
# shellcheck disable=SC1091
source "$ROOT_DIR/scripts/pdfium-runtime.sh"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(nproc)}"
prepare_cargo_target "$ROOT_DIR"

printf '\n\033[1;34m==> Preparing verified Pdfium runtime\033[0m\n'
ostudy_prepare_pdfium
printf 'Pdfium %s: %s\n' "$OSTUDY_PDFIUM_BUILD" "$OSTUDY_PDFIUM_LIBRARY"

printf '\n\033[1;34m==> Frontend production build\033[0m\n'
pnpm run build

printf '\n\033[1;34m==> Standalone Tauri build\033[0m\n'
pnpm tauri build --debug --no-bundle --ci -- --locked
"$ROOT_DIR/scripts/verify-rust-dependencies.sh"
[[ -x "$OSTUDY_BINARY" ]]
ostudy_write_build_stamp "$(node -p "require('./package.json').version")"
printf '\nBuilt: %s\n' "$OSTUDY_BINARY"

#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/diagnostics.sh"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
ostudy_parse_diagnostic_args "$@"
if (( ${#OSTUDY_DIAGNOSTIC_REMAINDER[@]} > 0 )); then
  printf 'Usage: ./scripts/dev.sh [diagnostic options]\n' >&2
  ostudy_diagnostic_usage >&2
  exit 2
fi
cd "$ROOT_DIR"
exec pnpm tauri dev -- --locked

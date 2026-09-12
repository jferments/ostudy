#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/diagnostics.bash"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
DEFAULT_NAME="ostudy-diagnostics-$TIMESTAMP.tar.gz"
OUTPUT="${1:-$ROOT_DIR/$DEFAULT_NAME}"
if [[ -d "$OUTPUT" ]]; then
  OUTPUT="$OUTPUT/$DEFAULT_NAME"
fi
OUTPUT="$(realpath -m "$OUTPUT")"
TEMP_DIR="$(mktemp -d -t ostudy-diagnostics.XXXXXXXX)"
cleanup() { rm -rf -- "$TEMP_DIR"; }
trap cleanup EXIT
mkdir -p "$TEMP_DIR/logs"

{
  printf 'ostudy diagnostic export\n'
  printf 'Created (UTC): %s\n' "$TIMESTAMP"
  printf 'This archive intentionally excludes the SQLite library, cards, and managed images.\n'
} > "$TEMP_DIR/README.txt"

(
  set +e
  "$SCRIPT_DIR/doctor.sh" --native-smoke
  printf '\nExit status: %s\n' "$?"
) > "$TEMP_DIR/doctor.txt" 2>&1

{
  uname -a
  printf '\nDesktop: %s\n' "${XDG_CURRENT_DESKTOP:-unknown}"
  printf 'Session: %s\n' "${XDG_SESSION_TYPE:-unknown}"
  printf 'Display: %s\n' "${DISPLAY:-unset}"
} > "$TEMP_DIR/environment.txt"

if command -v git >/dev/null 2>&1 && git -C "$ROOT_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  {
    git -C "$ROOT_DIR" rev-parse HEAD
    git -C "$ROOT_DIR" status --short
    git -C "$ROOT_DIR" remote -v | sed -E 's#(https://)[^/@]+@#\1REDACTED@#g'
  } > "$TEMP_DIR/git.txt" 2>&1
  # CI diagnostics include only the immutable published source, never the
  # runner's working tree, credentials, toolchains, or learner storage. This
  # makes exact-source inspection possible without reconstructing old sessions.
  # Ordinary local diagnostic exports keep their existing smaller contract.
  if [[ "${CI:-}" == true && "${QUALIFIED_SHA:-}" =~ ^[0-9a-f]{40}$ ]] \
    && [[ "$(git -C "$ROOT_DIR" rev-parse HEAD)" == "$QUALIFIED_SHA" ]]; then
    git -C "$ROOT_DIR" archive --format=tar.gz "$QUALIFIED_SHA" \
      > "$TEMP_DIR/published-source.tar.gz"
    {
      printf 'sourceSha=%s\n' "$QUALIFIED_SHA"
      printf 'treeSha=%s\n' "$(git -C "$ROOT_DIR" rev-parse "$QUALIFIED_SHA^{tree}")"
      sha256sum "$TEMP_DIR/published-source.tar.gz" | sed "s#$TEMP_DIR/##"
    } > "$TEMP_DIR/published-source-manifest.txt"
    printf 'CI-only: published-source.tar.gz is the exact committed source, not an edited candidate.\n' \
      >> "$TEMP_DIR/README.txt"
  fi
fi

cp "$ROOT_DIR/package.json" "$ROOT_DIR/versions.env" "$TEMP_DIR/"
LOG_PATH="$(ostudy_resolve_application_diagnostic_log)"
PREVIOUS_LOG_PATH="$(dirname -- "$LOG_PATH")/ostudy.previous.jsonl"
if [[ -f "$LOG_PATH" ]]; then
  tail -n 1000 -- "$LOG_PATH" > "$TEMP_DIR/logs/ostudy.jsonl"
fi
if [[ -f "$PREVIOUS_LOG_PATH" ]]; then
  tail -n 1000 -- "$PREVIOUS_LOG_PATH" > "$TEMP_DIR/logs/ostudy.previous.jsonl"
fi

mkdir -p "$(dirname -- "$OUTPUT")"
tar -czf "$OUTPUT" -C "$TEMP_DIR" .
printf 'Diagnostic archive created: %s\n' "$OUTPUT"

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

# Exercise the terminal-takeover shell handshake without requiring a real TTY.
# /dev/null substitutes for /dev/tty while a delayed acknowledgement models the
# Rust parent restoring Ratatui before the command is allowed to start.
takeover_ack="$TMP_DIR/takeover.ready"
(
  sleep 0.2
  : > "$takeover_ack"
) &
ack_pid=$!
takeover_output="$(
  OSTUDY_CONSOLE_EVENTS=1 \
  OSTUDY_CONSOLE_TAKEOVER_ACK="$takeover_ack" \
  OSTUDY_CONSOLE_TTY=/dev/null \
    bash -Eeuo pipefail -c '
      # shellcheck disable=SC1090
      source "$1"
      ostudy_console_takeover "fixture terminal ownership" true
    ' _ "$SCRIPT_DIR/lib/console-events.bash"
)"
wait "$ack_pid"
grep -Fqx $'@@OSTUDY:TUI@@\ttakeover\tstart\tfixture terminal ownership' <<< "$takeover_output"
grep -Fqx $'@@OSTUDY:TUI@@\ttakeover\tend\t0\tfixture terminal ownership' <<< "$takeover_output"
[[ ! -e "$takeover_ack" ]]

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

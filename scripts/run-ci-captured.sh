#!/usr/bin/env bash
set -Eeuo pipefail

if (( $# < 2 )); then
  echo "Usage: run-ci-captured.sh <log-file> <command> [args...]" >&2
  exit 2
fi

LOG_FILE="$1"
shift
SUCCESS_TAIL_BYTES="${OSTUDY_CI_SUCCESS_TAIL_BYTES:-4096}"
FAILURE_TAIL_BYTES="${OSTUDY_CI_FAILURE_TAIL_BYTES:-32768}"
HEARTBEAT_SECONDS="${OSTUDY_CI_HEARTBEAT_SECONDS:-60}"
PHASE="$(basename -- "$LOG_FILE" .log)"

is_positive_integer() {
  [[ "$1" =~ ^[1-9][0-9]*$ ]]
}

if ! is_positive_integer "$SUCCESS_TAIL_BYTES" ||
  ! is_positive_integer "$FAILURE_TAIL_BYTES" ||
  ! is_positive_integer "$HEARTBEAT_SECONDS"; then
  echo "OSTUDY_CI_SUCCESS_TAIL_BYTES, OSTUDY_CI_FAILURE_TAIL_BYTES, and OSTUDY_CI_HEARTBEAT_SECONDS must be positive integers." >&2
  exit 2
fi

mkdir -p -- "$(dirname -- "$LOG_FILE")"
: > "$LOG_FILE"

heartbeat_pid=""
stop_heartbeat() {
  if [[ -n "$heartbeat_pid" ]]; then
    kill "$heartbeat_pid" >/dev/null 2>&1 || true
    wait "$heartbeat_pid" 2>/dev/null || true
    heartbeat_pid=""
  fi
}

heartbeat_loop() {
  local started_at="$SECONDS"
  local previous_size=0
  local current_size delta_size elapsed
  local heartbeat_sleep_pid=""

  trap 'if [[ -n "$heartbeat_sleep_pid" ]]; then kill "$heartbeat_sleep_pid" >/dev/null 2>&1 || true; fi; exit 0' TERM INT
  while true; do
    sleep "$HEARTBEAT_SECONDS" &
    heartbeat_sleep_pid="$!"
    wait "$heartbeat_sleep_pid" || exit 0
    heartbeat_sleep_pid=""
    if [[ -f "$LOG_FILE" ]]; then
      current_size="$(wc -c < "$LOG_FILE" | tr -d '[:space:]')"
    else
      current_size=0
    fi
    [[ -n "$current_size" ]] || current_size=0
    if (( current_size < previous_size )); then
      previous_size=0
    fi
    delta_size=$((current_size - previous_size))
    elapsed=$((SECONDS - started_at))
    printf 'WAIT Qualification still running; phase=%s elapsed=%ss log_bytes=%s delta_bytes=%s.\n' \
      "$PHASE" "$elapsed" "$current_size" "$delta_size"
    previous_size="$current_size"
  done
}

trap stop_heartbeat EXIT
heartbeat_loop &
heartbeat_pid="$!"

set +e
"$@" > "$LOG_FILE" 2>&1
status=$?
set -e

stop_heartbeat
trap - EXIT

log_size="$(wc -c < "$LOG_FILE" | tr -d '[:space:]')"
if [[ -z "$log_size" ]]; then
  log_size=0
fi

if (( status == 0 )); then
  printf 'OK   Qualification command passed; full output retained in %s (%s bytes).\n' "$LOG_FILE" "$log_size"
  if (( log_size > 0 )); then
    printf '%s\n' '--- bounded success tail ---'
    tail -c "$SUCCESS_TAIL_BYTES" "$LOG_FILE"
    printf '\n%s\n' '--- end bounded success tail ---'
  fi
  exit 0
fi

printf 'ERROR Qualification command failed with exit %s; full output retained in %s (%s bytes).\n' \
  "$status" "$LOG_FILE" "$log_size" >&2
if (( log_size > 0 )); then
  printf '%s\n' '--- bounded failure tail ---' >&2
  tail -c "$FAILURE_TAIL_BYTES" "$LOG_FILE" >&2
  printf '\n%s\n' '--- end bounded failure tail ---' >&2
fi
exit "$status"

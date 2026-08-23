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

is_positive_integer() {
  [[ "$1" =~ ^[1-9][0-9]*$ ]]
}

if ! is_positive_integer "$SUCCESS_TAIL_BYTES" || ! is_positive_integer "$FAILURE_TAIL_BYTES"; then
  echo "OSTUDY_CI_SUCCESS_TAIL_BYTES and OSTUDY_CI_FAILURE_TAIL_BYTES must be positive integers." >&2
  exit 2
fi

mkdir -p -- "$(dirname -- "$LOG_FILE")"
: > "$LOG_FILE"

set +e
"$@" > "$LOG_FILE" 2>&1
status=$?
set -e

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

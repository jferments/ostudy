#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

SERIAL=""
OUTPUT=""
PREVIOUS=0
PACKAGE_NAME="io.github.jferments.ostudy.dev"

usage() {
  cat <<'USAGE'
Usage: ./scripts/android-diagnostics.sh [options]

Print the app-private structured diagnostic JSONL from an installed ostudy
Android debug build. The command uses `adb run-as`; no root access is required.

Options:
  --serial <adb-serial>  Select one device when more than one is connected.
  --previous             Read ostudy.previous.jsonl instead of ostudy.jsonl.
  --output <path>        Write the log to a local file instead of stdout.
  -h, --help             Show this help.
USAGE
}

while (($#)); do
  case "$1" in
    --serial)
      shift
      SERIAL="${1:?--serial requires an adb device serial}"
      ;;
    --previous) PREVIOUS=1 ;;
    --output)
      shift
      OUTPUT="${1:?--output requires a local path}"
      ;;
    -h|--help) usage; exit 0 ;;
    *)
      printf 'Unknown Android diagnostics option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

cd "$ROOT_DIR"

adb_args=()
if [[ -n "$SERIAL" ]]; then
  adb_args=(-s "$SERIAL")
fi
mapfile -t devices < <(adb devices | awk 'NR > 1 && $2 == "device" { print $1 }')
if [[ -n "$SERIAL" ]]; then
  if ! printf '%s\n' "${devices[@]}" | grep -Fxq -- "$SERIAL"; then
    printf 'ADB device %s is not connected and authorized.\n' "$SERIAL" >&2
    exit 1
  fi
elif ((${#devices[@]} != 1)); then
  printf 'Expected exactly one connected Android device, found %s. Use --serial when needed.\n' \
    "${#devices[@]}" >&2
  printf '%s\n' "${devices[@]}" >&2
  exit 1
fi

filename="ostudy.jsonl"
if ((PREVIOUS == 1)); then
  filename="ostudy.previous.jsonl"
fi

# Tauri's Android app-data location is intentionally treated as an internal
# detail. Search only inside this debuggable package's sandbox so changes in the
# platform path mapping do not break diagnostics retrieval.
remote_path="$(
  adb "${adb_args[@]}" shell run-as "$PACKAGE_NAME" sh -c \
    "find . -type f -path '*/diagnostics/$filename' -print -quit" \
    | tr -d '\r'
)"
if [[ -z "$remote_path" ]]; then
  cat >&2 <<EOF
No $filename was found in the $PACKAGE_NAME sandbox.
Launch the debug app once before retrieving diagnostics. If `run-as` reports
that the package is not debuggable, install the normal ostudy debug APK first.
EOF
  exit 1
fi

if [[ -n "$OUTPUT" ]]; then
  mkdir -p "$(dirname -- "$OUTPUT")"
  adb "${adb_args[@]}" exec-out run-as "$PACKAGE_NAME" cat "$remote_path" > "$OUTPUT"
  printf 'OK   Wrote Android diagnostics to %s\n' "$OUTPUT"
else
  adb "${adb_args[@]}" exec-out run-as "$PACKAGE_NAME" cat "$remote_path"
fi

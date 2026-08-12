#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

DEVICE=""
HOST=""
OPEN_STUDIO=0

usage() {
  cat <<'USAGE'
Usage: ./scripts/android-dev.sh [options]

Options:
  --device <name-or-serial> Select a connected emulator or physical device.
  --host <address>          Address exposed to the Android device for Vite.
  --open-studio             Open the generated project in Android Studio.
  -h, --help                Show this help.

For an emulator, Tauri normally resolves the development host automatically.
For a physical phone, pass your computer's LAN address, for example
--host 192.168.1.25, and permit TCP port 1420 through the development firewall.
USAGE
}

while (($#)); do
  case "$1" in
    --device) shift; DEVICE="${1:?--device requires a value}" ;;
    --host) shift; HOST="${1:?--host requires a value}" ;;
    --open-studio) OPEN_STUDIO=1 ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown Android development option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

cd "$ROOT_DIR"
bash ./scripts/android-doctor.sh
android_write_local_properties

args=(android dev)
[[ -n "$DEVICE" ]] && args+=(--device "$DEVICE")
[[ -n "$HOST" ]] && args+=(--host "$HOST")
((OPEN_STUDIO == 1)) && args+=(--open)

exec pnpm tauri "${args[@]}"

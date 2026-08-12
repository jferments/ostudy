#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

MODE=debug
TARGET=aarch64
FORMAT=apk
SMOKE_BRIDGE=0

usage() {
  cat <<'USAGE'
Usage: ./scripts/android-build.sh [options]

Options:
  --debug                 Build a debuggable APK (default).
  --release               Build an optimized release artifact.
  --target <abi>          aarch64, armv7, i686, or x86_64 (default: aarch64).
  --apk                    Build an APK (default).
  --aab                    Build an Android App Bundle.
  --smoke-bridge           Include the test-only emulator bridge. Never use for release.
  -h, --help               Show this help.
USAGE
}

while (($#)); do
  case "$1" in
    --debug) MODE=debug ;;
    --release) MODE=release ;;
    --target)
      shift
      TARGET="${1:?--target requires an ABI}"
      ;;
    --apk) FORMAT=apk ;;
    --aab) FORMAT=aab ;;
    --smoke-bridge) SMOKE_BRIDGE=1 ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown Android build option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

case "$TARGET" in
  aarch64|armv7|i686|x86_64) ;;
  *) printf 'Unsupported Android target: %s\n' "$TARGET" >&2; exit 2 ;;
esac
if [[ "$MODE" == release && "$SMOKE_BRIDGE" == 1 ]]; then
  printf 'The Android smoke bridge is forbidden in release artifacts.\n' >&2
  exit 2
fi

cd "$ROOT_DIR"
bash ./scripts/android-doctor.sh
android_write_local_properties

args=(android build "--$FORMAT" --target "$TARGET")
if [[ "$MODE" == debug ]]; then
  args+=(--debug)
fi

if [[ "$SMOKE_BRIDGE" == 1 ]]; then
  export VITE_OSTUDY_ANDROID_TEST_BRIDGE=1
else
  unset VITE_OSTUDY_ANDROID_TEST_BRIDGE || true
fi

printf 'Building ostudy Android %s %s for %s...\n' "$MODE" "$FORMAT" "$TARGET"
pnpm tauri "${args[@]}"

output_root="$ROOT_DIR/src-tauri/gen/android/app/build/outputs"
printf '\nGenerated Android artifacts:\n'
find "$output_root" -type f \( -name '*.apk' -o -name '*.aab' \) -print | sort

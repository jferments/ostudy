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
  --target <abi|all>      aarch64, armv7, i686, x86_64, or all.
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
  aarch64|armv7|i686|x86_64|all) ;;
  *) printf 'Unsupported Android target: %s\n' "$TARGET" >&2; exit 2 ;;
esac
if [[ "$MODE" == release && "$SMOKE_BRIDGE" == 1 ]]; then
  printf 'The Android smoke bridge is forbidden in release artifacts.\n' >&2
  exit 2
fi

cd "$ROOT_DIR"
bash ./scripts/android-doctor.sh
node ./scripts/verify-external-file-streaming.mjs
android_write_local_properties

# The Tauri CLI rewrites parts of the committed generated Android project while
# materializing a build. Preserve the reviewed manifest template verbatim so a
# build does not leave the source checkout dirty, including when the build fails.
manifest="$ROOT_DIR/src-tauri/gen/android/app/src/main/AndroidManifest.xml"
manifest_backup="$(mktemp)"
cp -- "$manifest" "$manifest_backup"
restore_generated_sources() {
  cp -- "$manifest_backup" "$manifest"
  rm -f -- "$manifest_backup"
}
trap restore_generated_sources EXIT

targets=("$TARGET")
if [[ "$TARGET" == all ]]; then
  targets=(aarch64 armv7 i686 x86_64)
fi
args=(android build "--$FORMAT" --target "${targets[@]}")
if [[ "$MODE" == debug ]]; then
  args+=(--debug)
fi

if [[ "$SMOKE_BRIDGE" == 1 ]]; then
  export VITE_OSTUDY_ANDROID_TEST_BRIDGE=1
else
  unset VITE_OSTUDY_ANDROID_TEST_BRIDGE || true
fi

printf 'Building ostudy Android %s %s for %s...\n' "$MODE" "$FORMAT" "${targets[*]}"
pnpm tauri "${args[@]}"

# Tauri's beforeBuildCommand has just generated the exact frontend distribution
# consumed by this Android build. Validate the smoke-only chunk here instead of
# grepping the APK: Tauri may embed/compress frontend assets inside the native
# bundle, so JavaScript literals are not a reliable observable of APK contents.
mapfile -t bridge_chunks < <(
  find "$ROOT_DIR/dist/assets" -maxdepth 1 -type f -name 'androidTestBridge-*.js' -print 2>/dev/null | sort
)
if [[ "$SMOKE_BRIDGE" == 1 ]]; then
  if ((${#bridge_chunks[@]} != 1)); then
    printf 'Android smoke frontend must contain exactly one test bridge chunk; found %s.\n' \
      "${#bridge_chunks[@]}" >&2
    exit 1
  fi
  if ! grep -a -q '__OSTUDY_ANDROID_TEST__' "${bridge_chunks[0]}"; then
    printf 'Android smoke frontend bridge chunk is missing its runtime marker: %s\n' \
      "${bridge_chunks[0]}" >&2
    exit 1
  fi
  printf 'OK   Android smoke frontend includes %s.\n' "${bridge_chunks[0]#"$ROOT_DIR/"}"
else
  if ((${#bridge_chunks[@]} != 0)) || grep -R -a -q '__OSTUDY_ANDROID_TEST__' "$ROOT_DIR/dist"; then
    printf 'Normal Android frontend unexpectedly contains the emulator-only test bridge.\n' >&2
    exit 1
  fi
  printf 'OK   Android %s frontend excludes the emulator-only test bridge.\n' "$MODE"
fi

output_root="$ROOT_DIR/src-tauri/gen/android/app/build/outputs"
printf '\nGenerated Android artifacts:\n'
find "$output_root" -type f \( -name '*.apk' -o -name '*.aab' \) -print | sort

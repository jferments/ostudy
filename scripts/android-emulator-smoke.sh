#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

KEEP_EMULATOR=0
WIPE_DATA=1
SERIAL="${ANDROID_SERIAL:-}"
ARTIFACT_DIR="${OSTUDY_ANDROID_SMOKE_ARTIFACTS:-$ROOT_DIR/artifacts/android-smoke}"

usage() {
  cat <<'USAGE'
Usage: ./scripts/android-emulator-smoke.sh [options]

Runs ostudy's instrumentation smoke suite in the official Android Emulator.
Waydroid is useful for informal manual checks but is not the qualification
runtime because it does not reproduce the standard emulator/device matrix.

Options:
  --serial <adb-serial>  Use an already-running Android device/emulator.
  --keep-emulator       Do not stop an emulator started by this script.
  --no-wipe             Reuse the AVD data image.
  --artifacts <dir>     Diagnostic output directory.
  -h, --help            Show this help.
USAGE
}

while (($#)); do
  case "$1" in
    --serial) shift; SERIAL="${1:?--serial requires a value}" ;;
    --keep-emulator) KEEP_EMULATOR=1 ;;
    --no-wipe) WIPE_DATA=0 ;;
    --artifacts) shift; ARTIFACT_DIR="${1:?--artifacts requires a path}" ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown smoke-test option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

cd "$ROOT_DIR"
bash ./scripts/android-doctor.sh --emulator
android_write_local_properties
mkdir -p "$ARTIFACT_DIR"
rm -f "$ARTIFACT_DIR"/*

adb="$ANDROID_HOME/platform-tools/adb"
started_emulator=0
cleanup() {
  set +e
  if [[ -n "$SERIAL" ]]; then
    "$adb" -s "$SERIAL" logcat -d > "$ARTIFACT_DIR/logcat.txt" 2>&1
    "$adb" -s "$SERIAL" exec-out screencap -p > "$ARTIFACT_DIR/final-screen.png" 2>/dev/null
    "$adb" -s "$SERIAL" shell uiautomator dump /sdcard/ostudy-window.xml >/dev/null 2>&1
    "$adb" -s "$SERIAL" pull /sdcard/ostudy-window.xml "$ARTIFACT_DIR/window.xml" >/dev/null 2>&1
  fi
  if ((started_emulator == 1 && KEEP_EMULATOR == 0)); then
    bash ./scripts/android-avd.sh stop >/dev/null 2>&1
  fi
}
trap cleanup EXIT

if [[ -z "$SERIAL" ]]; then
  start_args=(start --headless)
  ((WIPE_DATA == 1)) && start_args+=(--wipe-data)
  bash ./scripts/android-avd.sh "${start_args[@]}"
  started_emulator=1
  SERIAL="$($adb devices | awk 'NR > 1 && $2 == "device" && $1 ~ /^emulator-/ { print $1; exit }')"
fi
if [[ -z "$SERIAL" ]]; then
  printf 'No Android emulator or device is available.\n' >&2
  exit 1
fi
export ANDROID_SERIAL="$SERIAL"

"$adb" -s "$SERIAL" logcat -c
printf 'Building the x86_64 Android smoke APK and instrumentation package...\n'
export VITE_OSTUDY_ANDROID_TEST_BRIDGE=1
bash ./scripts/android-build.sh --debug --apk --target x86_64 --smoke-bridge
(
  cd src-tauri/gen/android
  ./gradlew --no-daemon :app:assembleDebug :app:assembleDebugAndroidTest
)

app_apk="$({
  find src-tauri/gen/android/app/build/outputs/apk -type f -name '*.apk' \
    ! -path '*/androidTest/*' -printf '%T@ %p\n'
} | sort -nr | awk 'NR == 1 { $1=""; sub(/^ /, ""); print }')"
test_apk="$({
  find src-tauri/gen/android/app/build/outputs/apk/androidTest -type f -name '*.apk' \
    -printf '%T@ %p\n' 2>/dev/null || true
} | sort -nr | awk 'NR == 1 { $1=""; sub(/^ /, ""); print }')"
if [[ -z "$app_apk" || -z "$test_apk" ]]; then
  printf 'Could not locate the Android application and instrumentation APKs.\n' >&2
  exit 1
fi

bash ./scripts/verify-android-package.sh "$app_apk" debug | tee "$ARTIFACT_DIR/package-verification.txt"
"$adb" -s "$SERIAL" install -r -t "$app_apk" | tee "$ARTIFACT_DIR/install-app.txt"
"$adb" -s "$SERIAL" install -r -t "$test_apk" | tee "$ARTIFACT_DIR/install-test.txt"

runner="$($adb -s "$SERIAL" shell pm list instrumentation | tr -d '\r' |
  sed -n 's/^instrumentation:\([^ ]*\) (target=io.github.jferments.ostudy.dev)$/\1/p' | head -n 1)"
if [[ -z "$runner" ]]; then
  printf 'The ostudy Android instrumentation runner was not registered.\n' >&2
  "$adb" -s "$SERIAL" shell pm list instrumentation >&2
  exit 1
fi

run_case() {
  local method="$1"
  local log="$ARTIFACT_DIR/${method}.txt"
  printf 'Running Android smoke case %s...\n' "$method"
  "$adb" -s "$SERIAL" shell am instrument -w -r \
    -e class "io.github.jferments.ostudy.AndroidSmokeTest#$method" \
    "$runner" | tee "$log"
  grep -q '^OK (' "$log" || {
    printf 'Android smoke case failed: %s\n' "$method" >&2
    exit 1
  }
}

"$adb" -s "$SERIAL" shell pm clear io.github.jferments.ostudy.dev >/dev/null
run_case a_seedLibraryPracticeAndAnalytics
"$adb" -s "$SERIAL" shell am force-stop io.github.jferments.ostudy.dev
run_case b_restartPreservesLibraryAndReviewHistory
run_case c_documentsStubAndAndroidBackReturnToDecks
run_case d_reloadsIntoInterruptedPracticeSession
"$adb" -s "$SERIAL" shell am force-stop io.github.jferments.ostudy.dev
run_case e_processRestartKeepsInterruptedPracticeSession

printf '\nOK   Android Emulator smoke suite passed on %s.\n' "$SERIAL"
printf 'Diagnostics: %s\n' "$ARTIFACT_DIR"

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
APP_ID="io.github.jferments.ostudy.dev"
APP_APK=""
TEST_APK=""
AVD_NAME="$ANDROID_PHONE_AVD"
SYSTEM_IMAGE="$ANDROID_PHONE_SYSTEM_IMAGE"

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
  --app-apk <path>      Reuse a prebuilt smoke-enabled application APK.
  --test-apk <path>     Reuse the matching prebuilt instrumentation APK.
  --avd-name <name>     Use another AVD name.
  --system-image <pkg>  System image used when the requested AVD must be created.
  -h, --help            Show this help.
USAGE
}

while (($#)); do
  case "$1" in
    --serial) shift; SERIAL="${1:?--serial requires a value}" ;;
    --keep-emulator) KEEP_EMULATOR=1 ;;
    --no-wipe) WIPE_DATA=0 ;;
    --artifacts) shift; ARTIFACT_DIR="${1:?--artifacts requires a path}" ;;
    --app-apk) shift; APP_APK="${1:?--app-apk requires a path}" ;;
    --test-apk) shift; TEST_APK="${1:?--test-apk requires a path}" ;;
    --avd-name) shift; AVD_NAME="${1:?--avd-name requires a value}" ;;
    --system-image) shift; SYSTEM_IMAGE="${1:?--system-image requires a value}" ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown smoke-test option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

if [[ -n "$APP_APK" || -n "$TEST_APK" ]]; then
  if [[ -z "$APP_APK" || -z "$TEST_APK" ]]; then
    printf 'Both --app-apk and --test-apk are required when reusing prebuilt smoke artifacts.\n' >&2
    exit 2
  fi
  APP_APK="$(readlink -f "$APP_APK")"
  TEST_APK="$(readlink -f "$TEST_APK")"
fi

cd "$ROOT_DIR"
printf 'Android emulator parallelism: %s Gradle workers, %s Cargo jobs.\n' \
  "$OSTUDY_GRADLE_WORKERS" "$CARGO_BUILD_JOBS"
bash ./scripts/android-doctor.sh --emulator
android_write_local_properties
rm -rf -- "$ARTIFACT_DIR"
mkdir -p "$ARTIFACT_DIR"

RESULTS_TSV="$ARTIFACT_DIR/android-smoke-results.tsv"
printf 'case\tstatus\tprerequisite\tduration_ms\tlog\n' > "$RESULTS_TSV"

adb="$ANDROID_HOME/platform-tools/adb"
started_emulator=0
sanitize_field() {
  printf '%s' "$1" | tr '\t\r\n' '   '
}
record_result() {
  local method="$1" status="$2" prerequisite="$3" duration_ms="$4" log="$5"
  printf '%s\t%s\t%s\t%s\t%s\n' \
    "$(sanitize_field "$method")" \
    "$(sanitize_field "$status")" \
    "$(sanitize_field "$prerequisite")" \
    "$(sanitize_field "$duration_ms")" \
    "$(sanitize_field "$log")" \
    >> "$RESULTS_TSV"
}

cleanup() {
  local status=$?
  set +e
  local api_level="" page_size=""
  if [[ -n "$SERIAL" ]]; then
    "$adb" -s "$SERIAL" logcat -d > "$ARTIFACT_DIR/logcat.txt" 2>&1
    "$adb" -s "$SERIAL" exec-out screencap -p > "$ARTIFACT_DIR/final-screen.png" 2>/dev/null
    "$adb" -s "$SERIAL" shell uiautomator dump /sdcard/ostudy-window.xml >/dev/null 2>&1
    "$adb" -s "$SERIAL" pull /sdcard/ostudy-window.xml "$ARTIFACT_DIR/window.xml" >/dev/null 2>&1
    api_level="$($adb -s "$SERIAL" shell getprop ro.build.version.sdk 2>/dev/null | tr -d '\r')"
    page_size="$($adb -s "$SERIAL" shell getconf PAGE_SIZE 2>/dev/null | tr -d '\r')"
  fi
  node "$SCRIPT_DIR/summarize-android-smoke.mjs" \
    --results "$RESULTS_TSV" \
    --artifact-dir "$ARTIFACT_DIR" \
    --output "$ARTIFACT_DIR/android-smoke-results.json" \
    --exit-code "$status" \
    --serial "$SERIAL" \
    --api-level "$api_level" \
    --page-size "$page_size" \
    > "$ARTIFACT_DIR/result-summary.txt" 2>&1 || true
  if ((started_emulator == 1 && KEEP_EMULATOR == 0)); then
    bash ./scripts/android-avd.sh stop >/dev/null 2>&1
  fi
  trap - EXIT
  exit "$status"
}
trap cleanup EXIT

if [[ -z "$SERIAL" ]]; then
  start_args=(start --headless --name "$AVD_NAME" --system-image "$SYSTEM_IMAGE")
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
if [[ -z "$APP_APK" ]]; then
  printf 'Building x86_64 Android smoke APK and instrumentation package...\n'
  export VITE_OSTUDY_ANDROID_TEST_BRIDGE=1
  bash ./scripts/android-build.sh --debug --apk --target x86_64 --smoke-bridge
  bash ./scripts/android-gradle-postbuild.sh instrumentation-package
  APP_APK="$({
    find src-tauri/gen/android/app/build/outputs/apk -type f -name '*.apk' \
      ! -path '*/androidTest/*' -printf '%T@ %p\n' 2>/dev/null || true
  } | sort -nr | awk 'NR == 1 { $1=""; sub(/^ /, ""); print }')"
  TEST_APK="$({
    find src-tauri/gen/android/app/build/outputs/apk/androidTest -type f -name '*.apk' \
      -printf '%T@ %p\n' 2>/dev/null || true
  } | sort -nr | awk 'NR == 1 { $1=""; sub(/^ /, ""); print }')"
else
  printf 'Reusing prebuilt Android smoke APKs.\n'
fi
if [[ ! -f "$APP_APK" || ! -f "$TEST_APK" ]]; then
  printf 'Could not locate Android application and instrumentation APKs.\n' >&2
  exit 1
fi

bash ./scripts/verify-android-package.sh "$APP_APK" debug | tee "$ARTIFACT_DIR/package-verification.txt"
"$adb" -s "$SERIAL" install -r -t "$APP_APK" | tee "$ARTIFACT_DIR/install-app.txt"
"$adb" -s "$SERIAL" install -r -t "$TEST_APK" | tee "$ARTIFACT_DIR/install-test.txt"

runner="$($adb -s "$SERIAL" shell pm list instrumentation | tr -d '\r' |
  sed -n 's/^instrumentation:\([^ ]*\) (target=io.github.jferments.ostudy.dev)$/\1/p' | head -n 1)"
if [[ -z "$runner" ]]; then
  printf 'The ostudy Android instrumentation runner was not registered.\n' >&2
  "$adb" -s "$SERIAL" shell pm list instrumentation >&2
  exit 1
fi

dump_case_logcat() {
  local method="$1"
  local crash_log="$ARTIFACT_DIR/${method}-logcat.txt"
  "$adb" -s "$SERIAL" logcat -d -v threadtime > "$crash_log" 2>&1 || true
  printf '\n--- Android crash diagnostics for %s ---\n' "$method" >&2
  grep -E -i \
    'FATAL EXCEPTION|Fatal signal|Abort message|SIGABRT|SIGSEGV|AndroidRuntime|panicked at|Rust panic|Process: io\.github\.jferments\.ostudy|io\.github\.jferments\.ostudy|FORTIFY' \
    "$crash_log" | tail -n 220 >&2 || true
  printf '%s\n' "--- Last 220 logcat lines for $method ---" >&2
  tail -n 220 "$crash_log" >&2 || true
  printf '%s\n' "--- End Android crash diagnostics ---" >&2
  printf 'Full case logcat: %s\n' "$crash_log" >&2
}

run_case() {
  local method="$1"
  local log="$ARTIFACT_DIR/${method}.txt"
  local started finished duration instrument_status test_class
  test_class="io.github.jferments.ostudy.AndroidSmokeTest"
  if [[ "$method" == "h_documentProviderRoundTripStreamsImportsAndExport" ]]; then
    test_class="io.github.jferments.ostudy.AndroidDocumentsProviderSmokeTest"
  fi

  "$adb" -s "$SERIAL" shell am force-stop "$APP_ID" >/dev/null 2>&1 || true
  "$adb" -s "$SERIAL" logcat -c

  printf 'Running Android smoke case %s...\n' "$method"
  started="$(date +%s%3N)"
  set +e
  "$adb" -s "$SERIAL" shell am instrument -w -r \
    -e waitForActivitiesToComplete false \
    -e class "$test_class#$method" \
    "$runner" | tee "$log"
  instrument_status="${PIPESTATUS[0]}"
  set -e
  finished="$(date +%s%3N)"
  duration=$((finished - started))

  if ((instrument_status != 0)) || ! grep -q '^OK (' "$log"; then
    record_result "$method" "FAIL" "" "$duration" "${method}.txt"
    printf 'Android smoke case failed: %s (instrumentation exit %s)\n' \
      "$method" "$instrument_status" >&2
    dump_case_logcat "$method"
    return 1
  fi
  record_result "$method" "PASS" "" "$duration" "${method}.txt"
}

failed_cases=()
skipped_cases=()
record_case() {
  local method="$1"
  if run_case "$method"; then
    return 0
  fi
  failed_cases+=("$method")
  return 1
}
skip_case() {
  local method="$1"
  local reason="$2"
  skipped_cases+=("$method: $reason")
  record_result "$method" "SKIP" "$reason" "" ""
  printf 'SKIP Android smoke case %s: %s\n' "$method" "$reason"
}

# Reset once, then preserve SQLite/app-private state across fresh processes.
"$adb" -s "$SERIAL" shell pm clear "$APP_ID" >/dev/null

launch_ok=1
record_case a0_appLaunchesAndTestBridgeIsReady || launch_ok=0
if ((launch_ok == 0)); then
  for method in \
    a_seedLibraryPracticeAndAnalytics \
    b_restartPreservesLibraryAndReviewHistory \
    c_documentsStubAndAndroidBackReturnToFlashcardsDecks \
    c_nestedDeckAndEditorBackReturnThroughTheStack \
    d_reloadsIntoInterruptedPracticeSession \
    e_processRestartKeepsInterruptedPracticeSession \
    f_studyContentImportPickerCancelsCleanly \
    f_imageImportPickerCancelsCleanly \
    f_pdfImportPickerCancelsCleanly \
    f_libraryBackupImportPickerCancelsCleanly \
    f_deckExportPickerCancelsCleanly \
    g_sequentialPickerCancellationReleasesNativeOperationSlot \
    h_documentProviderRoundTripStreamsImportsAndExport
  do
    skip_case "$method" "the application/bridge launch prerequisite failed"
  done
else
  seed_ok=1
  record_case a_seedLibraryPracticeAndAnalytics || seed_ok=0
  record_case c_documentsStubAndAndroidBackReturnToFlashcardsDecks || true

  if ((seed_ok == 1)); then
    record_case b_restartPreservesLibraryAndReviewHistory || true
    record_case c_nestedDeckAndEditorBackReturnThroughTheStack || true
  else
    skip_case b_restartPreservesLibraryAndReviewHistory "library seeding failed"
    skip_case c_nestedDeckAndEditorBackReturnThroughTheStack "the seeded deck is unavailable"
  fi

  restoration_ok=1
  record_case d_reloadsIntoInterruptedPracticeSession || restoration_ok=0
  if ((restoration_ok == 1)); then
    record_case e_processRestartKeepsInterruptedPracticeSession || true
  else
    skip_case e_processRestartKeepsInterruptedPracticeSession \
      "the interrupted practice-session prerequisite failed"
  fi

  for method in \
    f_studyContentImportPickerCancelsCleanly \
    f_imageImportPickerCancelsCleanly \
    f_pdfImportPickerCancelsCleanly \
    f_libraryBackupImportPickerCancelsCleanly \
    f_deckExportPickerCancelsCleanly \
    g_sequentialPickerCancellationReleasesNativeOperationSlot \
    h_documentProviderRoundTripStreamsImportsAndExport
  do
    record_case "$method" || true
  done
fi

if ((${#skipped_cases[@]} > 0)); then
  printf '\nAndroid smoke cases skipped because a prerequisite failed:\n'
  printf '  - %s\n' "${skipped_cases[@]}"
fi
if ((${#failed_cases[@]} > 0)); then
  printf '\nAndroid Emulator smoke suite failed in %s case(s):\n' "${#failed_cases[@]}" >&2
  printf '  - %s\n' "${failed_cases[@]}" >&2
  printf 'Diagnostics: %s\n' "$ARTIFACT_DIR" >&2
  exit 1
fi

printf '\nOK   Android Emulator smoke suite passed on %s.\n' "$SERIAL"
printf 'Diagnostics: %s\n' "$ARTIFACT_DIR"

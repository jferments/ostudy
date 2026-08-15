#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

ROUNDS=4
SERIAL="${ANDROID_SERIAL:-}"
ARTIFACT_DIR="${OSTUDY_PICKER_DIAGNOSTIC_ARTIFACTS:-$ROOT_DIR/artifacts/android-picker-cancellation-diagnostic}"
APP_ID="io.github.jferments.ostudy.dev"
TEST_CLASS="io.github.jferments.ostudy.AndroidPickerCancellationDiagnosticTest#pickerCancellationBoundary"
requested_purposes=()
started_emulator=0

usage() {
  cat <<'USAGE'
Usage: bash ./scripts/android-picker-cancellation-diagnostic.sh [options]

Builds the Android smoke APK/instrumentation package once, then repeatedly runs
one diagnostic cancellation test in a fresh app process. The diagnostic reports
whether OstudyPlatformPlugin.pendingImport is still occupied when JavaScript
fails to settle, separating a missing ActivityResult callback from a later
invoke/JNI/WebView completion failure.

Options:
  --rounds <n>          Iterations per picker purpose (default: 4).
  --purpose <name>      Restrict to one purpose; may be repeated.
                        Supported: study_content, image, pdf_document, library_backup.
  --serial <adb-serial> Reuse an already-running Android emulator/device.
  --artifacts <dir>     Diagnostic output directory.
  -h, --help            Show this help.
USAGE
}

while (($#)); do
  case "$1" in
    --rounds)
      shift
      ROUNDS="${1:?--rounds requires a positive integer}"
      ;;
    --purpose)
      shift
      requested_purposes+=("${1:?--purpose requires a value}")
      ;;
    --serial)
      shift
      SERIAL="${1:?--serial requires a value}"
      ;;
    --artifacts)
      shift
      ARTIFACT_DIR="${1:?--artifacts requires a path}"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'Unknown diagnostic option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

[[ "$ROUNDS" =~ ^[1-9][0-9]*$ ]] || {
  printf '--rounds must be a positive integer.\n' >&2
  exit 2
}

supported_purposes=(study_content image pdf_document library_backup)
if ((${#requested_purposes[@]} == 0)); then
  requested_purposes=("${supported_purposes[@]}")
fi
for purpose in "${requested_purposes[@]}"; do
  valid=0
  for supported in "${supported_purposes[@]}"; do
    if [[ "$purpose" == "$supported" ]]; then
      valid=1
      break
    fi
  done
  if ((valid == 0)); then
    printf 'Unsupported picker purpose: %s\n' "$purpose" >&2
    exit 2
  fi
done

cd "$ROOT_DIR"
printf 'Android picker cancellation diagnostic\n'
printf '  rounds/purpose: %s\n' "$ROUNDS"
printf '  purposes:       %s\n' "${requested_purposes[*]}"
printf '  artifacts:      %s\n\n' "$ARTIFACT_DIR"

bash ./scripts/android-doctor.sh --emulator
android_write_local_properties
rm -rf -- "$ARTIFACT_DIR"
mkdir -p "$ARTIFACT_DIR"

adb="$ANDROID_HOME/platform-tools/adb"
cleanup() {
  set +e
  if ((started_emulator == 1)); then
    bash ./scripts/android-avd.sh stop >/dev/null 2>&1
  fi
}
trap cleanup EXIT

if [[ -z "$SERIAL" ]]; then
  bash ./scripts/android-avd.sh start --headless --wipe-data
  started_emulator=1
  SERIAL="$($adb devices | awk 'NR > 1 && $2 == "device" && $1 ~ /^emulator-/ { print $1; exit }')"
fi
if [[ -z "$SERIAL" ]]; then
  printf 'No Android emulator/device is available.\n' >&2
  exit 1
fi
export ANDROID_SERIAL="$SERIAL"
printf 'Using Android device: %s\n' "$SERIAL"

printf '\nBuilding the x86_64 smoke APK and instrumentation package once...\n'
export VITE_OSTUDY_ANDROID_TEST_BRIDGE=1
bash ./scripts/android-build.sh --debug --apk --target x86_64 --smoke-bridge
bash ./scripts/android-gradle-postbuild.sh instrumentation-package

app_apk="$ROOT_DIR/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk"
test_apk="$({
  find src-tauri/gen/android/app/build/outputs/apk/androidTest/universal/debug \
    -type f -name '*.apk' -printf '%T@ %p\n' 2>/dev/null || true
} | sort -nr | awk 'NR == 1 { $1=""; sub(/^ /, ""); print }')"
if [[ ! -f "$app_apk" || -z "$test_apk" || ! -f "$test_apk" ]]; then
  printf 'Could not locate the application/instrumentation APKs.\n' >&2
  exit 1
fi

bash ./scripts/verify-android-package.sh "$app_apk" debug \
  | tee "$ARTIFACT_DIR/package-verification.txt"
"$adb" -s "$SERIAL" install -r -t "$app_apk" \
  | tee "$ARTIFACT_DIR/install-app.txt"
"$adb" -s "$SERIAL" install -r -t "$test_apk" \
  | tee "$ARTIFACT_DIR/install-test.txt"

runner="$($adb -s "$SERIAL" shell pm list instrumentation | tr -d '\r' |
  sed -n 's/^instrumentation:\([^ ]*\) (target=io.github.jferments.ostudy.dev)$/\1/p' | head -n 1)"
if [[ -z "$runner" ]]; then
  printf 'The ostudy Android instrumentation runner was not registered.\n' >&2
  "$adb" -s "$SERIAL" shell pm list instrumentation >&2
  exit 1
fi

printf 'Instrumentation runner: %s\n\n' "$runner"

failures=0
passes=0
summary="$ARTIFACT_DIR/summary.txt"
: > "$summary"

for purpose in "${requested_purposes[@]}"; do
  for ((round = 1; round <= ROUNDS; round++)); do
    case_id="${purpose}-$(printf '%02d' "$round")"
    result_log="$ARTIFACT_DIR/${case_id}.txt"
    logcat_file="$ARTIFACT_DIR/${case_id}-logcat.txt"
    trace_file="$ARTIFACT_DIR/${case_id}-trace.txt"

    printf '==> %s round %d/%d\n' "$purpose" "$round" "$ROUNDS"
    "$adb" -s "$SERIAL" shell am force-stop "$APP_ID" >/dev/null 2>&1 || true
    "$adb" -s "$SERIAL" shell pm clear "$APP_ID" >/dev/null
    "$adb" -s "$SERIAL" logcat -c

    set +e
    "$adb" -s "$SERIAL" shell am instrument -w -r \
      -e waitForActivitiesToComplete false \
      -e pickerPurpose "$purpose" \
      -e class "$TEST_CLASS" \
      "$runner" | tee "$result_log"
    instrument_status="${PIPESTATUS[0]}"
    set -e

    "$adb" -s "$SERIAL" logcat -d -v threadtime > "$logcat_file" 2>&1 || true
    grep -E \
      'OstudyPickerDiag|TestRunner|ActivityTaskManager|LifecycleMonitor|documentsui|DocumentsUI' \
      "$logcat_file" > "$trace_file" 2>/dev/null || true

    if ((instrument_status == 0)) && grep -q '^OK (' "$result_log"; then
      printf 'PASS %s\n\n' "$case_id"
      printf 'PASS %s\n' "$case_id" >> "$summary"
      passes=$((passes + 1))
    else
      printf 'FAIL %s (instrumentation exit %s)\n' "$case_id" "$instrument_status" >&2
      printf 'FAIL %s\n' "$case_id" >> "$summary"
      failures=$((failures + 1))
      printf '%s\n' "--- diagnostic trace: $trace_file ---" >&2
      tail -n 120 "$trace_file" >&2 || true
      printf '\n'
    fi
  done
done

printf '\nPicker cancellation diagnostic complete.\n'
printf '  passes:   %d\n' "$passes"
printf '  failures: %d\n' "$failures"
printf '  summary:  %s\n' "$summary"
printf '  traces:   %s\n' "$ARTIFACT_DIR"

if ((failures > 0)); then
  printf '\nOne or more diagnostic runs reproduced the cancellation failure.\n' >&2
  printf 'Inspect each failing *-trace.txt for nativePendingImport=true/false.\n' >&2
  exit 1
fi

printf '\nNo cancellation failure reproduced in this diagnostic sample.\n'

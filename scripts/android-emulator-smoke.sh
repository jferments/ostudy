#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

# The public qualification entrypoint owns the postbuild contract even though
# setup and installation are kept in a bounded sourced helper.
android_instrumentation_postbuild() {
  bash ./scripts/android-gradle-postbuild.sh instrumentation-package
}

# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/android-emulator-smoke-setup.bash"

run_case() {
  local method="$1"
  local class_name="${2:-AndroidSmokeTest}"
  local case_name="${class_name}-${method}"
  local log="$ARTIFACT_DIR/${case_name}.txt"

  # The Tauri event loop owns Activity/native teardown. AndroidJUnitRunner's
  # default activity finisher races Wry teardown, so instrumentation opts out
  # and this shell gives each case a fresh process with durable app data intact.
  "$adb" -s "$SERIAL" shell am force-stop "$APP_ID" >/dev/null 2>&1 || true
  "$adb" -s "$SERIAL" logcat -c

  printf 'Running Android smoke case %s#%s...\n' "$class_name" "$method"
  set +e
  timeout --signal=INT --kill-after=30s 180s \
    "$adb" -s "$SERIAL" shell am instrument -w -r \
    -e waitForActivitiesToComplete false \
    -e class "io.github.jferments.ostudy.${class_name}#${method}" \
    "$runner" | tee "$log"
  local instrument_status="${PIPESTATUS[0]}"
  set -e

  if ((instrument_status != 0)) || ! grep -q '^OK (' "$log"; then
    "$adb" -s "$SERIAL" shell am force-stop "$APP_ID" >/dev/null 2>&1 || true
    printf 'Android smoke case failed: %s#%s (instrumentation exit %s)\n' \
      "$class_name" "$method" "$instrument_status" >&2
    dump_case_logcat "$case_name"
    return 1
  fi
}

failed_cases=()
skipped_cases=()
record_case() {
  local method="$1"
  local class_name="${2:-AndroidSmokeTest}"
  if run_case "$method" "$class_name"; then
    return 0
  fi
  failed_cases+=("${class_name}#${method}")
  return 1
}
skip_case() {
  local method="$1"
  local reason="$2"
  skipped_cases+=("$method: $reason")
  printf 'SKIP Android smoke case %s: %s\n' "$method" "$reason"
}

# Reset the sandbox once, then preserve SQLite/app-private state across fresh
# processes. Independent cases continue after a failure; only cases with an
# explicit failed prerequisite are skipped. This exposes multiple defects from
# one expensive APK build without turning dependency failures into noise.
"$adb" -s "$SERIAL" shell pm clear "$APP_ID" >/dev/null

# Native regression cases are deliberately executed, not merely compiled into
# the instrumentation APK. These cover the Android boundaries most likely to
# regress without a visible desktop failure: insets, bundled ML Kit OCR, and
# the Rust-to-Kotlin native plugin routing used during application startup.
record_case contentRootOwnsSystemBarAndDisplayCutoutInsets AndroidInsetsTest || true
record_case activityRequestsResizeSemanticsForTheSoftwareKeyboard AndroidInsetsTest || true
record_case bundledMlKitRecognizesHighContrastEnglishTextOffline AndroidOcrSmokeTest || true

launch_ok=1
if ! run_case rustPlatformSnapshotReachesDocumentAndQrNativeRoutes AndroidPlatformRoutingTest; then
  failed_cases+=("AndroidPlatformRoutingTest#rustPlatformSnapshotReachesDocumentAndQrNativeRoutes")
  launch_ok=0
elif ! run_case a0_appLaunchesAndTestBridgeIsReady; then
  failed_cases+=("AndroidSmokeTest#a0_appLaunchesAndTestBridgeIsReady")
  launch_ok=0
fi
if ((launch_ok == 0)); then
  for method in \
    a1_freshLibraryUsesQuietNavigation \
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
    g_sequentialPickerCancellationReleasesNativeOperationSlot
  do
    skip_case "$method" "the native platform/application launch prerequisite failed"
  done
else
  record_case a1_freshLibraryUsesQuietNavigation || true

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
    g_sequentialPickerCancellationReleasesNativeOperationSlot
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

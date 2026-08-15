#!/usr/bin/env bash
# Android qualification commands shared by fail-fast and diagnostic execution.
# Intentionally option-neutral when sourced.

OSTUDY_ANDROID_GATE_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
OSTUDY_ANDROID_GATE_ROOT="$(cd -- "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/android-env.sh"

ostudy_android_gate_project_contracts() {
  node "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/verify-android-project.mjs"
  node "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/verify-external-file-streaming.mjs"
}

ostudy_android_gate_smoke_frontend() {
  bash "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/android-smoke-frontend-preflight.sh"
}

ostudy_android_gate_build_arm64_debug() {
  bash "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/android-build.sh" --debug --apk --target aarch64
}

ostudy_android_gate_build_x86_smoke() {
  VITE_OSTUDY_ANDROID_TEST_BRIDGE=1 \
    bash "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/android-build.sh" --debug --apk --target x86_64 --smoke-bridge
}

ostudy_android_gate_gradle_contracts() {
  bash "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/android-gradle-contracts.sh"
}

ostudy_android_gate_jvm_tests() {
  bash "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/android-gradle-postbuild.sh" jvm-tests
}

ostudy_android_gate_lint() {
  bash "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/android-gradle-postbuild.sh" lint
}

ostudy_android_gate_instrumentation_package() {
  bash "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/android-gradle-postbuild.sh" instrumentation-package
}

ostudy_android_gate_test_packages() {
  bash "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/android-gradle-postbuild.sh" test-packages
}

ostudy_android_find_app_apk() {
  find "$OSTUDY_ANDROID_GATE_ROOT/src-tauri/gen/android/app/build/outputs/apk" \
    -type f -name '*.apk' ! -path '*/androidTest/*' -printf '%T@ %p\n' 2>/dev/null \
    | sort -nr \
    | awk 'NR == 1 { $1=""; sub(/^ /, ""); print }'
}

ostudy_android_find_test_apk() {
  find "$OSTUDY_ANDROID_GATE_ROOT/src-tauri/gen/android/app/build/outputs/apk/androidTest" \
    -type f -name '*.apk' -printf '%T@ %p\n' 2>/dev/null \
    | sort -nr \
    | awk 'NR == 1 { $1=""; sub(/^ /, ""); print }'
}

ostudy_android_gate_verify_latest_app() {
  local apk
  apk="$(ostudy_android_find_app_apk)"
  if [[ -z "$apk" || ! -f "$apk" ]]; then
    printf 'No Android application APK could be located for package verification.\n' >&2
    return 1
  fi
  bash "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/verify-android-package.sh" "$apk" debug
}

ostudy_android_gate_verify_test_packages() {
  local app_apk test_apk
  app_apk="$(ostudy_android_find_app_apk)"
  test_apk="$(ostudy_android_find_test_apk)"
  if [[ -z "$app_apk" || ! -f "$app_apk" ]]; then
    printf 'The x86_64 Android smoke application APK could not be located.\n' >&2
    return 1
  fi
  if [[ -z "$test_apk" || ! -f "$test_apk" ]]; then
    printf 'The Android instrumentation APK could not be located.\n' >&2
    return 1
  fi
  bash "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/verify-android-package.sh" "$app_apk" debug
  printf 'OK   Android instrumentation package is present: %s\n' "${test_apk#"$OSTUDY_ANDROID_GATE_ROOT/"}"
}

ostudy_android_gate_build_cleanliness() {
  bash "$OSTUDY_ANDROID_GATE_SCRIPT_DIR/verify-build-cleanliness.sh"
}

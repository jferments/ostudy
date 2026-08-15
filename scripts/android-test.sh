#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/qualification-gates.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-qualification-gates.sh"

RUN_EMULATOR=0
case "${1:-}" in
  "") ;;
  --emulator) RUN_EMULATOR=1 ;;
  -h|--help)
    printf 'Usage: ./scripts/android-test.sh [--emulator]\n'
    exit 0
    ;;
  *) printf 'Unknown option: %s\n' "$1" >&2; exit 2 ;;
esac

cd "$ROOT_DIR"
printf 'Android parallelism: %s logical workers (Cargo=%s, Vitest=%s, Gradle=%s).\n' \
  "$OSTUDY_ANDROID_JOBS" "$CARGO_BUILD_JOBS" "$VITEST_MAX_WORKERS" "$OSTUDY_GRADLE_WORKERS"

if ((RUN_EMULATOR == 1)); then
  bash ./scripts/android-doctor.sh --emulator
else
  bash ./scripts/android-doctor.sh
fi
android_write_local_properties

printf '\nInstalling frozen JavaScript dependencies...\n'
ostudy_gate_pnpm_install

printf '\nVerifying Android project contracts...\n'
ostudy_android_gate_project_contracts

printf '\nProvisioning the audited host PDFium fixture...\n'
ostudy_gate_pdfium_runtime
export OSTUDY_PDFIUM_PATH="$ROOT_DIR/src-tauri/resources/native/pdfium/libpdfium.so"

printf '\nType-checking shared frontend...\n'
ostudy_gate_typescript

printf '\nRunning Vitest with up to %s workers...\n' "$VITEST_MAX_WORKERS"
pnpm exec vitest run --maxWorkers "$VITEST_MAX_WORKERS"

printf '\nChecking shared Rust with %s compiler jobs...\n' "$CARGO_BUILD_JOBS"
ostudy_gate_cargo_check

printf '\nRunning shared Rust tests with %s test threads...\n' "$OSTUDY_ANDROID_JOBS"
ostudy_gate_rust_tests "$OSTUDY_ANDROID_JOBS"

# Catch missing or accidentally leaked smoke-bridge wiring before paying for a
# native Android build. This operates only on Vite output and therefore cannot
# be fooled by Tauri's APK asset embedding/compression.
printf '\nPreflighting Android smoke frontend boundaries...\n'
ostudy_android_gate_smoke_frontend

# Exercise the physical-device architecture before the emulator build. The
# x86_64 smoke build intentionally runs last because Tauri may reuse a common
# universal APK path; keeping x86_64 last leaves the artifacts used by emulator
# qualification intact.
printf '\nBuilding and verifying arm64 Android debug package...\n'
ostudy_android_gate_build_arm64_debug
ostudy_android_gate_verify_latest_app

# Tauri must initialize transient tauri.settings.gradle and the private plugin
# graph before any direct Gradle command is legal.
printf '\nBuilding x86_64 Android smoke package through Tauri...\n'
ostudy_android_gate_build_x86_smoke

printf '\nVerifying Tauri-initialized Gradle dependency/Kotlin contracts...\n'
ostudy_android_gate_gradle_contracts

printf '\nRunning Android JVM, Lint, and instrumentation-package qualification...\n'
ostudy_android_gate_test_packages
ostudy_android_gate_verify_test_packages

printf '\nVerifying Android build cleanliness...\n'
ostudy_android_gate_build_cleanliness

app_apk="$(ostudy_android_find_app_apk)"
test_apk="$(ostudy_android_find_test_apk)"
if ((RUN_EMULATOR == 1)); then
  printf '\nRunning emulator smoke against the already-built APKs...\n'
  bash ./scripts/android-emulator-smoke.sh \
    --app-apk "$app_apk" \
    --test-apk "$test_apk"
  ostudy_android_gate_build_cleanliness
fi

printf 'OK   Android shared checks, arm64 package, Tauri-first Gradle/Lint tests, x86_64 instrumentation package%s passed.\n' \
  "$([[ "$RUN_EMULATOR" == 1 ]] && printf ', and emulator smoke' || true)"

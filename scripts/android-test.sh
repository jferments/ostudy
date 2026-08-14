#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

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
pnpm install --frozen-lockfile
./scripts/install-native-deps.sh --pdfium-only
export OSTUDY_PDFIUM_PATH="$ROOT_DIR/src-tauri/resources/native/pdfium/libpdfium.so"
node ./scripts/verify-android-project.mjs

printf '\nType-checking shared frontend...\n'
pnpm exec tsc -b --pretty false

printf '\nRunning Vitest with up to %s workers...\n' "$VITEST_MAX_WORKERS"
pnpm exec vitest run --maxWorkers "$VITEST_MAX_WORKERS"

printf '\nChecking Rust with %s compiler jobs...\n' "$CARGO_BUILD_JOBS"
cargo check \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-targets \
  --all-features \
  --jobs "$CARGO_BUILD_JOBS"

printf '\nRunning Rust tests with %s compiler jobs and %s test threads...\n' \
  "$CARGO_BUILD_JOBS" "$OSTUDY_ANDROID_JOBS"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --jobs "$CARGO_BUILD_JOBS" \
  -- \
  --test-threads "$OSTUDY_ANDROID_JOBS"

printf '\nVerifying Android Kotlin/coroutines dependency compatibility...\n'
dependency_report="$(mktemp -t ostudy-android-dependencies.XXXXXXXX)"
trap 'rm -f -- "$dependency_report"' EXIT
(
  cd src-tauri/gen/android
  ./gradlew --no-daemon \
    --parallel \
    --max-workers "$OSTUDY_GRADLE_WORKERS" \
    :tauri-plugin-ostudy-platform:dependencies \
    --configuration debugCompileClasspath
) | tee "$dependency_report"
if ! grep -Eq 'kotlinx-coroutines-core-jvm:1\.8\.1' "$dependency_report"; then
  printf 'Android dependency resolution did not select the audited coroutines 1.8.1 line.\n' >&2
  exit 1
fi
if ! grep -Eq 'kotlin-stdlib:1\.9\.25' "$dependency_report"; then
  printf 'Android dependency resolution did not select the pinned Kotlin stdlib 1.9.25 line.\n' >&2
  exit 1
fi
if grep -Eq 'kotlin-stdlib:2\.[1-9][0-9]*\.' "$dependency_report"; then
  printf 'Android dependency resolution selected Kotlin 2.1+ metadata, which is incompatible with the pinned Kotlin 1.9.25 compiler.\n' >&2
  exit 1
fi
printf 'OK   Android Kotlin metadata dependency graph is compatible with Kotlin 1.9.25.\n'

if ((RUN_EMULATOR == 1)); then
  exec bash ./scripts/android-emulator-smoke.sh
fi

export VITE_OSTUDY_ANDROID_TEST_BRIDGE=1
bash ./scripts/android-build.sh --debug --apk --target x86_64 --smoke-bridge
(
  cd src-tauri/gen/android
  ./gradlew --no-daemon \
    --parallel \
    --max-workers "$OSTUDY_GRADLE_WORKERS" \
    :app:testDebugUnitTest \
    :app:assembleDebugAndroidTest
)
app_apk="$({
  find src-tauri/gen/android/app/build/outputs/apk -type f -name '*.apk' \
    ! -path '*/androidTest/*' -printf '%T@ %p\n'
} | sort -nr | awk 'NR == 1 { $1=""; sub(/^ /, ""); print }')"
if [[ -z "$app_apk" ]]; then
  printf 'The Android debug APK could not be located after the build.\n' >&2
  exit 1
fi
bash ./scripts/verify-android-package.sh "$app_apk" debug
printf 'OK   Android compile, parallel unit-test, instrumentation-package, and APK checks passed.\n'

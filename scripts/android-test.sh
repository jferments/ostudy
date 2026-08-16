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
node ./scripts/verify-android-project.mjs
node ./scripts/verify-android-plugin-routing.mjs
node ./scripts/verify-local-fonts.mjs
pnpm install --frozen-lockfile
pnpm run verify:fonts
node ./scripts/verify-external-file-streaming.mjs

./scripts/install-native-deps.sh --pdfium-only
export OSTUDY_PDFIUM_PATH="$ROOT_DIR/src-tauri/resources/native/pdfium/libpdfium.so"

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

# tauri.settings.gradle and the private-plugin inclusion it contains are
# transient Tauri CLI build state. They are intentionally ignored by Git and
# do not exist in a fresh checkout. Any direct Gradle qualification must run
# only after a Tauri Android build has initialized that state.
verify_gradle_contracts() (
  set -Eeuo pipefail

  generated_settings="$ROOT_DIR/src-tauri/gen/android/tauri.settings.gradle"
  if [[ ! -f "$generated_settings" ]]; then
    printf 'Tauri did not initialize the generated Android Gradle settings: %s\n' \
      "$generated_settings" >&2
    printf 'Run Android Gradle qualification only after the Tauri Android build step.\n' >&2
    exit 1
  fi

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
  printf 'OK   Android Kotlin metadata dependency graph resolves to Kotlin 1.9.25 and coroutines 1.8.1.\n'

  printf '\nCompile-checking the private Android platform plugin...\n'
  (
    cd src-tauri/gen/android
    ./gradlew --no-daemon \
      --parallel \
      --max-workers "$OSTUDY_GRADLE_WORKERS" \
      :tauri-plugin-ostudy-platform:compileDebugKotlin
  )
  printf 'OK   Android platform plugin Kotlin compilation passed.\n'
)

if ((RUN_EMULATOR == 1)); then
  bash ./scripts/android-emulator-smoke.sh
  verify_gradle_contracts
  printf 'OK   Android shared checks, emulator instrumentation, and Gradle contracts passed.\n'
  exit 0
fi

# Build Rust/JNI exactly once through Tauri. Besides producing the x86_64
# application, this initializes the transient Gradle plugin settings required
# by the direct Gradle checks and postbuild tasks below.
export VITE_OSTUDY_ANDROID_TEST_BRIDGE=1
bash ./scripts/android-build.sh --debug --apk --target x86_64 --smoke-bridge
verify_gradle_contracts

# Direct Gradle work that follows must reuse the Tauri build output instead of
# re-entering `android-studio-script` after Tauri's temporary CLI socket closes.
bash ./scripts/android-gradle-postbuild.sh test-packages

app_apk="$ROOT_DIR/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk"
test_apk="$({
  find src-tauri/gen/android/app/build/outputs/apk/androidTest/universal/debug \
    -type f -name '*.apk' -printf '%T@ %p\n' 2>/dev/null || true
} | sort -nr | awk 'NR == 1 { $1=""; sub(/^ /, ""); print }')"
if [[ ! -f "$app_apk" ]]; then
  printf 'The expected universal x86_64 Android debug APK is missing: %s\n' "$app_apk" >&2
  exit 1
fi
if [[ -z "$test_apk" || ! -f "$test_apk" ]]; then
  printf 'The UniversalDebug Android instrumentation APK could not be located.\n' >&2
  exit 1
fi
bash ./scripts/verify-android-package.sh "$app_apk" debug
printf 'OK   Android compile, parallel unit-test, Gradle-contract, instrumentation-package, and APK checks passed.\n'

#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/node-toolchain.bash"
ostudy_activate_node_toolchain
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/diagnostics.bash"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/check-state.sh"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
ostudy_parse_diagnostic_args "$@"

RUN_EMULATOR=0
for argument in "${OSTUDY_DIAGNOSTIC_REMAINDER[@]}"; do
  case "$argument" in
    --emulator) RUN_EMULATOR=1 ;;
    -h|--help)
      printf 'Usage: ./scripts/android-test.sh [--emulator] [diagnostic options]\n'
      ostudy_diagnostic_usage
      exit 0
      ;;
    *) printf 'Unknown option: %s\n' "$argument" >&2; exit 2 ;;
  esac
done

cd "$ROOT_DIR"

if [[ "${OSTUDY_ALLOW_QUALIFICATION_REUSE:-0}" == "1" ]] \
  && [[ "${OSTUDY_FORCE_CHECK:-0}" != "1" ]] \
  && ostudy_frontend_prerequisites_present \
  && ostudy_check_stamp_is_current static; then
  if ((RUN_EMULATOR == 0)) && ostudy_check_stamp_is_current android-compile; then
    reuse_sha="$(ostudy_check_stamp_source_sha android-compile || true)"
    printf '\n\033[1;32mREUSE Android compile/package qualification from %s: the current exact source passed static qualification and the Android product fingerprint is unchanged.\033[0m\n' \
      "${reuse_sha:-an earlier qualified source}"
    exit 0
  fi
  if ((RUN_EMULATOR == 1)) \
    && ostudy_check_stamp_is_current android-compile \
    && ostudy_check_stamp_is_current android-smoke; then
    reuse_sha="$(ostudy_check_stamp_source_sha android-smoke || true)"
    printf '\n\033[1;32mREUSE Android compile/package and emulator qualification from %s: the current exact source passed static qualification and the Android product fingerprint is unchanged.\033[0m\n' \
      "${reuse_sha:-an earlier qualified source}"
    exit 0
  fi
fi

WORKTREE_BASELINE="$(mktemp -t ostudy-android-worktree.XXXXXXXX)"
cleanup_worktree_baseline() { rm -f -- "$WORKTREE_BASELINE"; }
trap cleanup_worktree_baseline EXIT
ostudy_reset_worktree_baseline "$WORKTREE_BASELINE"
trap 'status=$?; ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "Android qualification" || true; exit "$status"' ERR

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
node ./scripts/verify-android-qr-scanner.mjs
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
  if ! grep -Eq 'play-services-code-scanner:16\.1\.0' "$dependency_report"; then
    printf 'Android dependency resolution did not include Google Code Scanner 16.1.0.\n' >&2
    exit 1
  fi
  if grep -Eqi 'journeyapps|zxing-android-embedded' "$dependency_report"; then
    printf 'The removed ZXing Embedded scanner remains in the Android dependency graph.\n' >&2
    exit 1
  fi
  printf 'OK   Android Kotlin, coroutines, and Google Code Scanner dependency contracts passed.\n'

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
  ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "Android Emulator smoke qualification"
  verify_gradle_contracts
  ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "Android Gradle contract verification"
  ostudy_write_check_stamp android-compile
  trap - ERR
  printf 'OK   Android shared checks, emulator instrumentation, and Gradle contracts passed.\n'
  exit 0
fi

# Build Rust/JNI exactly once through Tauri. Besides producing the x86_64
# application, this initializes the transient Gradle plugin settings required
# by the direct Gradle checks and postbuild tasks below.
export VITE_OSTUDY_ANDROID_TEST_BRIDGE=1
bash ./scripts/android-build.sh --debug --apk --target x86_64 --smoke-bridge
# android-build.sh owns restoration of the tracked generated sources that Tauri
# rewrites. Assert immediately so any newly introduced generator mutation is
# attributed to materialization rather than a later Gradle test.
ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "Tauri Android build materialization"
verify_gradle_contracts
ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "Android Gradle contract verification"

# Direct Gradle work that follows must reuse the Tauri build output instead of
# re-entering `android-studio-script` after Tauri's temporary CLI socket closes.
bash ./scripts/android-gradle-postbuild.sh test-packages
ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "Android Gradle postbuild"

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
ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "Android compile/package qualification"
ostudy_write_check_stamp android-compile
trap - ERR
printf 'OK   Android compile, parallel unit-test, Gradle-contract, instrumentation-package, and APK checks passed.\n'

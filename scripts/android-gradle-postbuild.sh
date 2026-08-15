#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

MODE="${1:-}"
case "$MODE" in
  test-packages|jvm-tests|lint|instrumentation-package) ;;
  -h|--help|"")
    cat <<'USAGE'
Usage: ./scripts/android-gradle-postbuild.sh <test-packages|jvm-tests|lint|instrumentation-package>

Runs Gradle-only work after `android-build.sh` has already built the x86_64
Rust library through the Tauri CLI. The wrapper deliberately excludes Tauri's
generated rustBuild tasks because those tasks require the temporary CLI socket
owned by the parent `tauri android build` process.

`test-packages` is the authoritative fail-fast bundle. The narrower modes let
diagnostic orchestration execute JVM tests, Android Lint, and instrumentation
assembly independently so one category does not hide unrelated failures.
USAGE
    [[ -n "$MODE" ]] && exit 0 || exit 2
    ;;
  *)
    printf 'Unknown Android Gradle postbuild mode: %s\n' "$MODE" >&2
    exit 2
    ;;
esac

cd "$ROOT_DIR"
android_write_local_properties

native_lib="$ROOT_DIR/src-tauri/gen/android/app/src/main/jniLibs/x86_64/libostudy_lib.so"
generated_settings="$ROOT_DIR/src-tauri/gen/android/tauri.settings.gradle"
if [[ ! -e "$native_lib" || ! -f "$generated_settings" ]]; then
  printf 'The Tauri-initialized x86_64 Android build state is incomplete.\n' >&2
  printf 'Run ./scripts/android-build.sh --debug --apk --target x86_64 first.\n' >&2
  exit 1
fi

# Direct Gradle invocations do not own the temporary Tauri CLI WebSocket used by
# `tauri android android-studio-script`. Reuse the already-built JNI library and
# keep the direct graph x86_64-only, while still allowing Gradle/Kotlin/JVM work
# to use the full configured worker pool.
gradle_args=(
  --no-daemon
  --parallel
  --max-workers "$OSTUDY_GRADLE_WORKERS"
  -PabiList=x86_64
  -ParchList=x86_64
  -PtargetList=x86_64
  -x :app:rustBuildUniversalDebug
  -x :app:rustBuildX86_64Debug
)

case "$MODE" in
  jvm-tests)
    tasks=(
      :tauri-plugin-ostudy-platform:testDebugUnitTest
      :app:testUniversalDebugUnitTest
    )
    ;;
  lint)
    tasks=(
      :tauri-plugin-ostudy-platform:lintDebug
      :app:lintUniversalDebug
    )
    ;;
  instrumentation-package)
    tasks=(
      :app:assembleUniversalDebugAndroidTest
    )
    ;;
  test-packages)
    tasks=(
      :tauri-plugin-ostudy-platform:testDebugUnitTest
      :tauri-plugin-ostudy-platform:lintDebug
      :app:testUniversalDebugUnitTest
      :app:lintUniversalDebug
      :app:assembleUniversalDebugAndroidTest
    )
    ;;
esac

printf 'Running Android Gradle postbuild (%s) with %s workers; Rust rebuild tasks are excluded.\n' \
  "$MODE" "$OSTUDY_GRADLE_WORKERS"
(
  cd src-tauri/gen/android
  ./gradlew "${gradle_args[@]}" "${tasks[@]}"
)

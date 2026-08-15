#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

MODE="${1:-}"
case "$MODE" in
  test-packages|instrumentation-package) ;;
  -h|--help|"")
    cat <<'USAGE'
Usage: ./scripts/android-gradle-postbuild.sh <test-packages|instrumentation-package>

Runs Gradle-only work after `android-build.sh` has already built the x86_64
Rust library through the Tauri CLI. The wrapper deliberately excludes Tauri's
generated rustBuild tasks because those tasks require the temporary CLI socket
owned by the parent `tauri android build` process.
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
if [[ ! -e "$native_lib" ]]; then
  printf 'The x86_64 Tauri native library is missing: %s\n' "$native_lib" >&2
  printf 'Run ./scripts/android-build.sh --debug --apk --target x86_64 first.\n' >&2
  exit 1
fi

generated_settings="$ROOT_DIR/src-tauri/gen/android/tauri.settings.gradle"
if [[ ! -f "$generated_settings" ]]; then
  printf 'The Tauri-generated Android Gradle settings are missing: %s\n' \
    "$generated_settings" >&2
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

tasks=(
  :app:assembleUniversalDebugAndroidTest
)
if [[ "$MODE" == test-packages ]]; then
  tasks=(
    :tauri-plugin-ostudy-platform:testDebugUnitTest
    :app:testUniversalDebugUnitTest
    :app:assembleUniversalDebugAndroidTest
  )
fi

printf 'Running Android Gradle postbuild (%s) with %s workers; Rust rebuild tasks are excluded.\n' \
  "$MODE" "$OSTUDY_GRADLE_WORKERS"
(
  cd src-tauri/gen/android
  ./gradlew "${gradle_args[@]}" "${tasks[@]}"
)

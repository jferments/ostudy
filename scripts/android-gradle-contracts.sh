#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

cd "$ROOT_DIR"
android_write_local_properties

generated_settings="$ROOT_DIR/src-tauri/gen/android/tauri.settings.gradle"
native_lib="$ROOT_DIR/src-tauri/gen/android/app/src/main/jniLibs/x86_64/libostudy_lib.so"
if [[ ! -f "$generated_settings" || ! -f "$native_lib" ]]; then
  printf 'Android Gradle contracts require Tauri-initialized x86_64 build state.\n' >&2
  printf 'Run ./scripts/android-build.sh --debug --apk --target x86_64 --smoke-bridge first.\n' >&2
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
    -PabiList=x86_64 \
    -ParchList=x86_64 \
    -PtargetList=x86_64 \
    -x :app:rustBuildUniversalDebug \
    -x :app:rustBuildX86_64Debug \
    :tauri-plugin-ostudy-platform:dependencies \
    --configuration debugCompileClasspath
) | tee "$dependency_report"
if ! grep -Eq "kotlinx-coroutines-core-jvm:${ANDROID_COROUTINES_VERSION}([[:space:]]|$)" "$dependency_report"; then
  printf 'Android dependency resolution did not select audited coroutines %s.\n' "$ANDROID_COROUTINES_VERSION" >&2
  exit 1
fi
if ! grep -Eq "kotlin-stdlib:${ANDROID_KOTLIN_VERSION}([[:space:]]|$)" "$dependency_report"; then
  printf 'Android dependency resolution did not select pinned Kotlin stdlib %s.\n' "$ANDROID_KOTLIN_VERSION" >&2
  exit 1
fi

printf '\nCompile-checking the private Android platform plugin...\n'
(
  cd src-tauri/gen/android
  ./gradlew --no-daemon \
    --parallel \
    --max-workers "$OSTUDY_GRADLE_WORKERS" \
    -PabiList=x86_64 \
    -ParchList=x86_64 \
    -PtargetList=x86_64 \
    -x :app:rustBuildUniversalDebug \
    -x :app:rustBuildX86_64Debug \
    :tauri-plugin-ostudy-platform:compileDebugKotlin
)

printf 'OK   Tauri-first Android Gradle dependency and Kotlin contracts passed.\n'

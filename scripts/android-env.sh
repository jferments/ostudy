#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"

if [[ -n "${ANDROID_HOME:-}" ]]; then
  resolved_android_home="$ANDROID_HOME"
elif [[ -n "${ANDROID_SDK_ROOT:-}" ]]; then
  resolved_android_home="$ANDROID_SDK_ROOT"
elif [[ -d "$HOME/Android/Sdk" ]]; then
  # Android Studio's default Linux SDK location. Reuse it so the runner does not
  # maintain a second multi-gigabyte SDK tree; ostudy still installs/validates its
  # own pinned platform, Build Tools, NDK, and emulator image side-by-side.
  resolved_android_home="$HOME/Android/Sdk"
else
  resolved_android_home="$HOME/.cache/ostudy/android-sdk"
fi
ANDROID_HOME="$resolved_android_home"
ANDROID_SDK_ROOT="$ANDROID_HOME"
NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/$ANDROID_NDK_VERSION}"

if [[ -n "${JAVA_HOME:-}" ]]; then
  resolved_java_home="$JAVA_HOME"
elif command -v javac >/dev/null 2>&1; then
  javac_path="$(readlink -f "$(command -v javac)")"
  resolved_java_home="$(dirname "$(dirname "$javac_path")")"
else
  resolved_java_home=""
fi

if [[ -n "${OSTUDY_ANDROID_JOBS:-}" ]]; then
  resolved_android_jobs="$OSTUDY_ANDROID_JOBS"
elif command -v nproc >/dev/null 2>&1; then
  resolved_android_jobs="$(nproc)"
else
  resolved_android_jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '1')"
fi
if [[ ! "$resolved_android_jobs" =~ ^[1-9][0-9]*$ ]]; then
  printf 'OSTUDY_ANDROID_JOBS must be a positive integer, found %q.\n' "$resolved_android_jobs" >&2
  return 1 2>/dev/null || exit 1
fi
OSTUDY_ANDROID_JOBS="$resolved_android_jobs"
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$OSTUDY_ANDROID_JOBS}"
VITEST_MAX_WORKERS="${VITEST_MAX_WORKERS:-$OSTUDY_ANDROID_JOBS}"
OSTUDY_GRADLE_WORKERS="${OSTUDY_GRADLE_WORKERS:-$OSTUDY_ANDROID_JOBS}"

JAVA_HOME="$resolved_java_home"
export ANDROID_HOME ANDROID_SDK_ROOT NDK_HOME JAVA_HOME
export OSTUDY_ANDROID_JOBS OSTUDY_GRADLE_WORKERS CARGO_BUILD_JOBS VITEST_MAX_WORKERS
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$ANDROID_HOME/cmdline-tools/latest/bin:$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

android_write_local_properties() {
  local generated_project="$ROOT_DIR/src-tauri/gen/android"
  if [[ ! -d "$generated_project" ]]; then
    return 0
  fi
  printf 'sdk.dir=%s\n' "${ANDROID_HOME//\/\\}" > "$generated_project/local.properties"
}

android_emit_github_environment() {
  : "${GITHUB_ENV:?GITHUB_ENV is required}"
  : "${GITHUB_PATH:?GITHUB_PATH is required}"
  {
    printf 'ANDROID_HOME=%s\n' "$ANDROID_HOME"
    printf 'ANDROID_SDK_ROOT=%s\n' "$ANDROID_SDK_ROOT"
    printf 'NDK_HOME=%s\n' "$NDK_HOME"
    printf 'JAVA_HOME=%s\n' "$JAVA_HOME"
    printf 'OSTUDY_ANDROID_JOBS=%s\n' "$OSTUDY_ANDROID_JOBS"
    printf 'OSTUDY_GRADLE_WORKERS=%s\n' "$OSTUDY_GRADLE_WORKERS"
    printf 'CARGO_BUILD_JOBS=%s\n' "$CARGO_BUILD_JOBS"
    printf 'VITEST_MAX_WORKERS=%s\n' "$VITEST_MAX_WORKERS"
  } >> "$GITHUB_ENV"
  {
    printf '%s\n' "$JAVA_HOME/bin"
    printf '%s\n' "$ANDROID_HOME/platform-tools"
    printf '%s\n' "$ANDROID_HOME/emulator"
    printf '%s\n' "$ANDROID_HOME/cmdline-tools/latest/bin"
    printf '%s\n' "$HOME/.local/bin"
    printf '%s\n' "$HOME/.cargo/bin"
  } >> "$GITHUB_PATH"
}

#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"

ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/.cache/ostudy/android-sdk}}"
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

JAVA_HOME="$resolved_java_home"
export ANDROID_HOME ANDROID_SDK_ROOT NDK_HOME JAVA_HOME
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

#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

REQUIRE_EMULATOR=0
case "${1:-}" in
  "") ;;
  --emulator) REQUIRE_EMULATOR=1 ;;
  *)
    printf 'Usage: ./scripts/android-doctor.sh [--emulator]\n' >&2
    exit 2
    ;;
esac

failures=0
ok() { printf 'OK   %s\n' "$*"; }
fail() { printf 'FAIL %s\n' "$*" >&2; failures=$((failures + 1)); }

require_command() {
  local command_name="$1"
  if command -v "$command_name" >/dev/null 2>&1; then
    ok "$command_name: $(command -v "$command_name")"
  else
    fail "$command_name is missing"
  fi
}

require_exact_version() {
  local command_name="$1"
  local expected="$2"
  local actual=""
  if ! command -v "$command_name" >/dev/null 2>&1; then
    return
  fi
  actual="$("$command_name" --version 2>/dev/null | sed 's/^v//')" || actual=""
  if [[ "$actual" == "$expected" ]]; then
    ok "$command_name $actual matches versions.env"
  else
    fail "$command_name must be $expected from versions.env, found ${actual:-unknown} at $(command -v "$command_name")"
  fi
}

printf 'ostudy Android development diagnostics\n\n'
for command_name in java javac node npm pnpm rustup rustc cargo python3 curl sha256sum; do
  require_command "$command_name"
done
require_exact_version node "$NODE_VERSION"
require_exact_version npm "$NPM_VERSION"
require_exact_version pnpm "$PNPM_VERSION"

if [[ -d "$ANDROID_HOME" ]]; then
  ok "Android SDK: $ANDROID_HOME"
else
  fail "Android SDK is missing at $ANDROID_HOME; run ./scripts/install-android-deps.sh"
fi

sdkmanager="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"
adb="$ANDROID_HOME/platform-tools/adb"
zipalign="$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS_VERSION/zipalign"

for tool in "$sdkmanager" "$adb" "$zipalign"; do
  if [[ -x "$tool" ]]; then
    ok "$tool"
  else
    fail "$tool is missing"
  fi
done

if [[ -n "$JAVA_HOME" && -x "$JAVA_HOME/bin/javac" ]]; then
  java_major="$($JAVA_HOME/bin/javac -version 2>&1 | sed -E 's/^javac ([0-9]+).*/\1/')"
  if [[ "$java_major" == "$ANDROID_JDK_MAJOR" ]]; then
    ok "JDK $java_major at $JAVA_HOME"
  else
    fail "Expected JDK $ANDROID_JDK_MAJOR, found $java_major at $JAVA_HOME"
  fi
else
  fail "JAVA_HOME does not identify a complete JDK"
fi

for required_path in \
  "$ANDROID_HOME/platforms/android-$ANDROID_COMPILE_SDK" \
  "$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS_VERSION" \
  "$NDK_HOME"; do
  if [[ -d "$required_path" ]]; then
    ok "$required_path"
  else
    fail "$required_path is missing"
  fi
done

installed_targets="$(rustup target list --installed 2>/dev/null || true)"
for target in \
  aarch64-linux-android \
  armv7-linux-androideabi \
  i686-linux-android \
  x86_64-linux-android; do
  if grep -qx "$target" <<< "$installed_targets"; then
    ok "Rust target $target"
  else
    fail "Rust target $target is missing"
  fi
done

if [[ -f "$ROOT_DIR/src-tauri/gen/android/gradlew" ]]; then
  ok "Generated Gradle wrapper is present"
else
  fail "Generated Android project is missing; run pnpm tauri android init --ci"
fi

if (( REQUIRE_EMULATOR == 1 )); then
  emulator="$ANDROID_HOME/emulator/emulator"
  avdmanager="$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager"
  if [[ -x "$emulator" ]]; then
    ok "Android Emulator: $emulator"
  else
    fail "Android Emulator is missing; run ./scripts/install-android-deps.sh --with-emulator"
  fi
  if [[ -x "$avdmanager" ]]; then
    ok "avdmanager: $avdmanager"
  else
    fail "avdmanager is missing"
  fi
  if [[ -e /dev/kvm && -r /dev/kvm && -w /dev/kvm ]]; then
    ok "KVM acceleration is available"
  else
    fail "KVM acceleration is unavailable to the current user"
  fi
fi

if (( failures > 0 )); then
  printf '\nFAILED: %d Android environment check(s) did not pass.\n' "$failures" >&2
  exit 1
fi
printf '\nOK   Android environment is ready.\n'

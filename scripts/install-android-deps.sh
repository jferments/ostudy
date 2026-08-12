#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

WITH_EMULATOR=0
case "${1:-}" in
  "") ;;
  --with-emulator) WITH_EMULATOR=1 ;;
  *)
    printf 'Usage: ./scripts/install-android-deps.sh [--with-emulator]\n' >&2
    exit 2
    ;;
esac

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'Missing required command: %s\n' "$1" >&2
    exit 1
  fi
}

for command_name in curl python3 sha256sum; do
  require_command "$command_name"
done

if [[ -z "$JAVA_HOME" || ! -x "$JAVA_HOME/bin/java" || ! -x "$JAVA_HOME/bin/javac" ]]; then
  printf 'JDK %s is required. Install it or set JAVA_HOME before continuing.\n' "$ANDROID_JDK_MAJOR" >&2
  exit 1
fi
java_major="$($JAVA_HOME/bin/javac -version 2>&1 | sed -E 's/^javac ([0-9]+).*/\1/')"
if [[ "$java_major" != "$ANDROID_JDK_MAJOR" ]]; then
  printf 'Expected JDK %s, found javac %s at %s.\n' "$ANDROID_JDK_MAJOR" "$java_major" "$JAVA_HOME" >&2
  exit 1
fi

mkdir -p "$ANDROID_HOME"
command_tools="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"
if [[ ! -x "$command_tools" ]]; then
  archive_name="commandlinetools-linux-${ANDROID_COMMANDLINE_TOOLS_BUILD}_latest.zip"
  archive_url="https://dl.google.com/android/repository/$archive_name"
  cache_dir="$HOME/.cache/ostudy/downloads"
  archive_path="$cache_dir/$archive_name"
  extraction_dir="$ANDROID_HOME/cmdline-tools/.extract-$ANDROID_COMMANDLINE_TOOLS_BUILD"
  mkdir -p "$cache_dir" "$ANDROID_HOME/cmdline-tools"

  if [[ ! -f "$archive_path" ]]; then
    printf 'Downloading Android command-line tools %s...\n' "$ANDROID_COMMANDLINE_TOOLS_BUILD"
    curl --fail --location --retry 4 --retry-delay 2 --output "$archive_path.partial" "$archive_url"
    mv "$archive_path.partial" "$archive_path"
  fi
  printf '%s  %s\n' "$ANDROID_COMMANDLINE_TOOLS_SHA256" "$archive_path" | sha256sum --check --status || {
    rm -f "$archive_path"
    printf 'Android command-line tools checksum verification failed.\n' >&2
    exit 1
  }

  rm -rf "$extraction_dir" "$ANDROID_HOME/cmdline-tools/latest"
  mkdir -p "$extraction_dir"
  python3 -m zipfile -e "$archive_path" "$extraction_dir"
  if [[ ! -d "$extraction_dir/cmdline-tools" ]]; then
    printf 'The Android command-line tools archive had an unexpected layout.\n' >&2
    exit 1
  fi
  mv "$extraction_dir/cmdline-tools" "$ANDROID_HOME/cmdline-tools/latest"
  rmdir "$extraction_dir"
fi

# Python's standard zip extractor intentionally does not restore Unix mode bits.
# Android's command-line archive contains shell launchers that must be executable.
chmod -R u+rwX "$ANDROID_HOME/cmdline-tools/latest"
find "$ANDROID_HOME/cmdline-tools/latest/bin" -maxdepth 1 -type f -exec chmod u+x {} +

sdkmanager="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"
packages=(
  "platform-tools"
  "platforms;android-$ANDROID_COMPILE_SDK"
  "build-tools;$ANDROID_BUILD_TOOLS_VERSION"
  "ndk;$ANDROID_NDK_VERSION"
)
if (( WITH_EMULATOR == 1 )); then
  packages+=("emulator" "$ANDROID_PHONE_SYSTEM_IMAGE")
fi

printf 'Accepting Android SDK licenses for the pinned packages...\n'
yes | "$sdkmanager" --sdk_root="$ANDROID_HOME" --licenses >/dev/null || true
printf 'Installing Android SDK packages into %s...\n' "$ANDROID_HOME"
"$sdkmanager" --sdk_root="$ANDROID_HOME" --install "${packages[@]}"
yes | "$sdkmanager" --sdk_root="$ANDROID_HOME" --licenses >/dev/null

if [[ ! -d "$NDK_HOME" ]]; then
  printf 'The pinned NDK was not installed at %s.\n' "$NDK_HOME" >&2
  exit 1
fi

android_write_local_properties
printf 'Android SDK ready.\n'
printf '  ANDROID_HOME=%s\n' "$ANDROID_HOME"
printf '  NDK_HOME=%s\n' "$NDK_HOME"
printf '  JAVA_HOME=%s\n' "$JAVA_HOME"

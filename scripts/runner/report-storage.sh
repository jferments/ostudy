#!/usr/bin/env bash
set -Eeuo pipefail

# Read-only, bounded storage inventory for the isolated CI guest. This avoids
# recursively scanning the whole filesystem while still covering the persistent
# locations that can grow between Actions runs.

timeout_seconds="${OSTUDY_STORAGE_DU_TIMEOUT_SECONDS:-20}"
if [[ ! "$timeout_seconds" =~ ^[1-9][0-9]*$ ]]; then
  printf 'OSTUDY_STORAGE_DU_TIMEOUT_SECONDS must be a positive integer.\n' >&2
  exit 2
fi

printf 'ostudy runner storage report\n'
df -h /

summary_file="${GITHUB_STEP_SUMMARY:-}"
if [[ -n "$summary_file" ]]; then
  {
    printf '\n### Runner storage\n\n'
    printf '| Area | Size | Path |\n'
    printf '| --- | ---: | --- |\n'
  } >> "$summary_file"
fi

measure() {
  local label="$1" path="$2"
  if [[ ! -e "$path" ]]; then
    printf 'STORAGE %-22s missing %s\n' "$label" "$path"
    return 0
  fi

  local human
  human="$(
    timeout "$timeout_seconds" du -shx -- "$path" 2>/dev/null | awk '{print $1}' || true
  )"
  if [[ -z "$human" ]]; then
    human="timed-out"
  fi
  printf 'STORAGE %-22s %10s %s\n' "$label" "$human" "$path"
  if [[ -n "$summary_file" ]]; then
    printf '| %s | %s | `%s` |\n' "$label" "$human" "$path" >> "$summary_file"
  fi
}

cargo_target="${OSTUDY_CARGO_TARGET_DIR:-$HOME/.cache/ostudy/cargo-target}"
measure "Cargo target" "$cargo_target"
measure "Cargo host debug" "$cargo_target/debug"
measure "Cargo host incremental" "$cargo_target/debug/incremental"
for rust_target in aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android; do
  measure "Cargo $rust_target" "$cargo_target/$rust_target"
  measure "Cargo $rust_target inc" "$cargo_target/$rust_target/debug/incremental"
done
measure "Gradle active" "${GRADLE_USER_HOME:-$HOME/.cache/gradle}"
if [[ "${GRADLE_USER_HOME:-$HOME/.cache/gradle}" != "$HOME/.cache/gradle" ]]; then
  measure "Gradle legacy" "$HOME/.cache/gradle"
fi
measure "pnpm store" "$HOME/.cache/pnpm"
measure "Cargo home" "${CARGO_HOME:-$HOME/.cargo}"
measure "Rustup" "${RUSTUP_HOME:-$HOME/.rustup}"
measure "Android SDK" "${ANDROID_HOME:-$HOME/Android/Sdk}"
measure "Android AVD" "${ANDROID_AVD_HOME:-$HOME/.android/avd}"
measure "Actions work" "$HOME/actions-runner/_work"
measure "Runner evidence" "$HOME/.local/state/ostudy/actions"
measure "Native deps" "${OSTUDY_NATIVE_DEPS_CACHE_DIR:-$HOME/.cache/ostudy/native-deps}"

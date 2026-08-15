#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

SYSTEM_IMAGE="$ANDROID_PHONE_SYSTEM_IMAGE"
while (($#)); do
  case "$1" in
    --system-image) shift; SYSTEM_IMAGE="${1:?--system-image requires a package}" ;;
    -h|--help)
      printf 'Usage: ./scripts/android-runner-doctor.sh [--system-image SDK_PACKAGE]\n'
      exit 0
      ;;
    *)
      printf 'Usage: ./scripts/android-runner-doctor.sh [--system-image SDK_PACKAGE]\n' >&2
      exit 2
      ;;
  esac
  shift
done

failures=0
ok() { printf 'OK   %s\n' "$*"; }
warn() { printf 'WARN %s\n' "$*"; }
fail() { printf 'FAIL %s\n' "$*" >&2; failures=$((failures + 1)); }

printf 'ostudy Android self-hosted runner diagnostics\n\n'

bash "$SCRIPT_DIR/android-doctor.sh" --emulator || failures=$((failures + 1))

adb="$ANDROID_HOME/platform-tools/adb"
emulator="$ANDROID_HOME/emulator/emulator"
sdkmanager="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"
system_image_path="$ANDROID_HOME/${SYSTEM_IMAGE//;/\/}"

if [[ -e /dev/kvm ]]; then
  if [[ -r /dev/kvm && -w /dev/kvm ]]; then
    ok "/dev/kvm is readable and writable by $(id -un)"
  else
    fail "/dev/kvm exists but $(id -un) cannot read and write it; add the runner service account to the kvm group and restart its login/service"
  fi
else
  fail "/dev/kvm is missing; the official x86_64 emulator cannot use Linux KVM acceleration on this runner"
fi

if command -v id >/dev/null 2>&1 && id -nG | tr ' ' '\n' | grep -qx kvm; then
  ok "runner account is a member of the kvm group"
else
  warn "runner account does not report kvm group membership"
fi

if [[ -x "$emulator" ]]; then
  accel_output="$($emulator -accel-check 2>&1 || true)"
  if grep -Eqi 'KVM.*(installed|usable)|accel.*(working|available)' <<< "$accel_output"; then
    ok "Android Emulator hardware acceleration is usable"
  else
    printf '%s\n' "$accel_output" >&2
    fail "Android Emulator acceleration check did not report usable KVM"
  fi
else
  fail "Android Emulator binary is missing at $emulator"
fi

for required_path in \
  "$ANDROID_HOME/platforms/android-$ANDROID_COMPILE_SDK" \
  "$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS_VERSION" \
  "$ANDROID_HOME/ndk/$ANDROID_NDK_VERSION" \
  "$system_image_path"; do
  if [[ -e "$required_path" ]]; then
    ok "$required_path"
  else
    fail "required Android CI package is missing: $required_path"
  fi
done

if [[ -x "$sdkmanager" ]]; then
  installed="$($sdkmanager --sdk_root="$ANDROID_HOME" --list_installed 2>/dev/null || true)"
  for package in \
    "platform-tools" \
    "platforms;android-$ANDROID_COMPILE_SDK" \
    "build-tools;$ANDROID_BUILD_TOOLS_VERSION" \
    "ndk;$ANDROID_NDK_VERSION" \
    "$SYSTEM_IMAGE"; do
    if grep -Fq "$package" <<< "$installed"; then
      ok "SDK package $package"
    else
      fail "sdkmanager does not report required package $package"
    fi
  done
fi

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

available_kib="$(awk '/MemAvailable:/ { print $2 }' /proc/meminfo 2>/dev/null || printf '0')"
if [[ "$available_kib" =~ ^[0-9]+$ ]] && (( available_kib >= 4 * 1024 * 1024 )); then
  ok "at least 4 GiB of memory is currently available"
else
  warn "less than 4 GiB of memory appears available; emulator + Rust/Gradle builds may be unstable"
fi

available_blocks="$(df -Pk "$ROOT_DIR" | awk 'NR == 2 { print $4 }')"
if [[ "$available_blocks" =~ ^[0-9]+$ ]] && (( available_blocks >= 15 * 1024 * 1024 )); then
  ok "at least 15 GiB of disk is free in the runner workspace filesystem"
else
  warn "less than 15 GiB of disk appears free in the runner workspace filesystem"
fi

if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
  ok "running inside GitHub Actions as ${RUNNER_NAME:-unknown-runner}"
  printf 'INFO Android compile workflow expects: self-hosted, linux, x64, ostudy-android\n'
  printf 'INFO emulator workflow additionally expects: kvm\n'
  printf 'INFO emulator image: %s\n' "$SYSTEM_IMAGE"
fi

if [[ -x "$adb" ]]; then
  "$adb" start-server >/dev/null 2>&1 || fail "adb server could not start"
fi

if (( failures > 0 )); then
  printf '\nFAILED: %d Android runner requirement(s) are not satisfied.\n' "$failures" >&2
  exit 1
fi

printf '\nOK   This host is ready for ostudy Android emulator jobs.\n'

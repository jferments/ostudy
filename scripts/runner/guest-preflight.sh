#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/../.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"

EXPECTED_RAM_GIB="${OSTUDY_VM_RAM_GIB:-64}"
EXPECTED_DISK_GIB="${OSTUDY_VM_DISK_GIB:-150}"
EXPECTED_SWAP_GIB="${OSTUDY_VM_SWAP_GIB:-16}"
EXPECTED_VCPUS="${OSTUDY_VM_VCPUS:-16}"
RUNNER_USER="${OSTUDY_VM_RUNNER_USER:-ostudy-runner}"
ANDROID_HOME_VALUE="${ANDROID_HOME:-/home/$RUNNER_USER/Android/Sdk}"

failures=0
warnings=0
ok() { printf 'OK    %s\n' "$*"; }
warn() { printf 'WARN  %s\n' "$*"; warnings=$((warnings + 1)); }
fail() { printf 'FAIL  %s\n' "$*" >&2; failures=$((failures + 1)); }

printf 'ostudy unified runner guest preflight\n\n'

if [[ "$(uname -s):$(uname -m)" == Linux:x86_64 ]]; then
  ok "guest is Linux x86_64"
else
  fail "guest must be Linux x86_64"
fi

mem_total_kib="$(awk '/^MemTotal:/ {print $2}' /proc/meminfo)"
mem_total_gib=$((mem_total_kib / 1024 / 1024))
minimum_ram=$((EXPECTED_RAM_GIB - 2))
if (( mem_total_gib >= minimum_ram )); then
  ok "guest RAM is ${mem_total_gib} GiB (requested ${EXPECTED_RAM_GIB} GiB)"
else
  fail "guest RAM is ${mem_total_gib} GiB; expected approximately ${EXPECTED_RAM_GIB} GiB"
fi

vcpus="$(nproc)"
if (( vcpus >= EXPECTED_VCPUS )); then
  ok "guest exposes $vcpus vCPUs"
else
  fail "guest exposes $vcpus vCPUs; expected at least $EXPECTED_VCPUS"
fi

root_size_kib="$(df -Pk / | awk 'NR == 2 {print $2}')"
root_size_gib=$((root_size_kib / 1024 / 1024))
minimum_disk=$((EXPECTED_DISK_GIB - 10))
if (( root_size_gib >= minimum_disk )); then
  ok "root filesystem is ${root_size_gib} GiB (requested ${EXPECTED_DISK_GIB} GiB virtual disk)"
else
  fail "root filesystem is only ${root_size_gib} GiB; expected at least ${minimum_disk} GiB"
fi

root_free_kib="$(df -Pk / | awk 'NR == 2 {print $4}')"
root_free_gib=$((root_free_kib / 1024 / 1024))
if (( root_free_gib >= 40 )); then
  ok "root filesystem has ${root_free_gib} GiB free"
elif (( root_free_gib >= 25 )); then
  warn "root filesystem has only ${root_free_gib} GiB free; prune caches before expensive qualification"
else
  fail "root filesystem has only ${root_free_gib} GiB free; minimum safe threshold is 25 GiB"
fi

swap_kib="$(awk 'NR > 1 {sum += $3} END {print sum + 0}' /proc/swaps)"
swap_gib=$((swap_kib / 1024 / 1024))
minimum_swap=$((EXPECTED_SWAP_GIB - 2))
if (( swap_gib >= minimum_swap )); then
  ok "persistent swap is ${swap_gib} GiB"
else
  fail "swap is ${swap_gib} GiB; expected approximately ${EXPECTED_SWAP_GIB} GiB"
fi
if grep -Eq '^/swapfile[[:space:]]+none[[:space:]]+swap' /etc/fstab; then
  ok "/swapfile is persistent in /etc/fstab"
else
  fail "/swapfile is not configured persistently in /etc/fstab"
fi

if grep -qm1 -E '\b(vmx|svm)\b' /proc/cpuinfo; then
  virtualization_flag="$(grep -m1 -Eo '\b(vmx|svm)\b' /proc/cpuinfo | head -n1)"
  ok "nested virtualization flag $virtualization_flag is visible inside the guest"
else
  fail "nested vmx/svm is not visible inside the guest; check host-passthrough CPU configuration"
fi

if [[ -e /dev/kvm && -r /dev/kvm && -w /dev/kvm ]]; then
  ok "/dev/kvm is usable by $(id -un)"
else
  fail "/dev/kvm is not readable/writable by $(id -un)"
fi

# cpu-checker's kvm-ok is useful as a host diagnostic, but it is not the
# authoritative gate for this nested Android runner. The direct /dev/kvm
# permission check above and the Android Emulator's own -accel-check below
# validate the actual capability this runner needs. Keep kvm-ok as additional
# evidence without rejecting an otherwise working nested-KVM guest.
if command -v kvm-ok >/dev/null 2>&1; then
  kvm_ok_output="$(kvm-ok 2>&1 || true)"
  if grep -Fq 'KVM acceleration can be used' <<< "$kvm_ok_output"; then
    ok "kvm-ok reports acceleration is available inside the guest"
  else
    warn "kvm-ok is inconclusive in this nested guest; direct KVM and Android Emulator checks remain authoritative"
    if [[ -n "$kvm_ok_output" ]]; then
      printf '%s\n' "$kvm_ok_output"
    fi
  fi
else
  warn "kvm-ok is unavailable; direct KVM and Android Emulator checks remain authoritative"
fi

if findmnt -rn -t 9p,virtiofs,nfs,nfs4,cifs,fuse.sshfs | grep -q .; then
  fail "guest contains a host/network filesystem mount; unified CI VM must not mount host files"
else
  ok "guest has no 9p, virtiofs, NFS, CIFS, or SSHFS mounts"
fi
for socket_path in /var/run/docker.sock /run/docker.sock /var/run/libvirt/libvirt-sock /run/libvirt/libvirt-sock; do
  if [[ -S "$socket_path" ]]; then
    fail "forbidden host-control socket is present: $socket_path"
  fi
done

if command -v ufw >/dev/null 2>&1 && sudo -n ufw status 2>/dev/null | grep -Fq 'Status: active'; then
  ok "UFW is active"
else
  warn "could not verify active UFW without administration privileges"
fi

check_exact_version() {
  local label="$1" actual="$2" expected="$3"
  if [[ "$actual" == "$expected" ]]; then
    ok "$label version is $actual"
  else
    fail "$label version is $actual; expected $expected"
  fi
}

# Keep the VM bootstrap contract at least as strict as the repository's
# known-good native Linux doctor. In particular, xvfb-run delegates Xauthority
# setup to xauth, so checking only for xvfb-run is not sufficient on a clean VM.
for command_name in \
  git curl jq node npm pnpm python3 rustup rustc cargo rustfmt cargo-clippy \
  pkg-config sqlite3 sha256sum tar xvfb-run xauth openbox xprop WebKitWebDriver tauri-driver \
  java javac; do
  command -v "$command_name" >/dev/null 2>&1 || fail "required command is missing: $command_name"
done

command -v node >/dev/null 2>&1 && check_exact_version Node "$(node --version | sed 's/^v//')" "$NODE_VERSION"
command -v npm >/dev/null 2>&1 && check_exact_version npm "$(npm --version)" "$NPM_VERSION"
command -v pnpm >/dev/null 2>&1 && check_exact_version pnpm "$(pnpm --version)" "$PNPM_VERSION"
command -v rustc >/dev/null 2>&1 && check_exact_version rustc "$(rustc --version | awk '{print $2}')" "$RUST_VERSION"
command -v cargo >/dev/null 2>&1 && check_exact_version cargo "$(cargo --version | awk '{print $2}')" "$CARGO_VERSION"
if command -v cargo >/dev/null 2>&1 && command -v tauri-driver >/dev/null 2>&1; then
  tauri_driver_version="$(
    cargo install --list |
      sed -n 's/^tauri-driver v\([^:]*\):$/\1/p' |
      head -n 1
  )"
  check_exact_version tauri-driver "$tauri_driver_version" "$TAURI_DRIVER_VERSION"
fi

if command -v pkg-config >/dev/null 2>&1; then
  if pkg-config --exists webkit2gtk-4.1; then
    ok "webkit2gtk-4.1 development files are available ($(pkg-config --modversion webkit2gtk-4.1))"
  else
    fail "webkit2gtk-4.1 development files are missing"
  fi
  if pkg-config --exists openssl; then
    ok "OpenSSL development files are available ($(pkg-config --modversion openssl))"
  else
    fail "OpenSSL development files are missing"
  fi
fi

if command -v dpkg-query >/dev/null 2>&1; then
  if dpkg-query -W -f='${Status}' libxdo-dev 2>/dev/null | grep -Fxq 'install ok installed'; then
    ok "libxdo-dev is installed"
  else
    fail "libxdo-dev is missing; native Linux qualification must match scripts/setup-ubuntu.sh"
  fi
fi

if command -v javac >/dev/null 2>&1; then
  java_major="$(javac -version 2>&1 | sed -E 's/^javac ([0-9]+).*/\1/')"
  check_exact_version JDK "$java_major" "$ANDROID_JDK_MAJOR"
fi

for target in aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android; do
  if rustup target list --installed | grep -Fxq "$target"; then
    ok "Rust target installed: $target"
  else
    fail "Rust target missing: $target"
  fi
done

required_android_paths=(
  "$ANDROID_HOME_VALUE/platform-tools/adb"
  "$ANDROID_HOME_VALUE/emulator/emulator"
  "$ANDROID_HOME_VALUE/cmdline-tools/latest/bin/sdkmanager"
  "$ANDROID_HOME_VALUE/platforms/android-$ANDROID_COMPILE_SDK"
  "$ANDROID_HOME_VALUE/build-tools/$ANDROID_BUILD_TOOLS_VERSION"
  "$ANDROID_HOME_VALUE/ndk/$ANDROID_NDK_VERSION"
  "$ANDROID_HOME_VALUE/${ANDROID_PHONE_SYSTEM_IMAGE//;/\/}"
)
for path in "${required_android_paths[@]}"; do
  if [[ -e "$path" ]]; then
    ok "Android component present: $path"
  else
    fail "Android component missing: $path"
  fi
done

if [[ -x "$ANDROID_HOME_VALUE/emulator/emulator" ]]; then
  acceleration="$($ANDROID_HOME_VALUE/emulator/emulator -accel-check 2>&1 || true)"
  if grep -Eqi 'KVM.*(installed|usable)|accel.*(working|available)' <<< "$acceleration"; then
    ok "Android Emulator reports usable KVM acceleration"
  else
    printf '%s\n' "$acceleration" >&2
    fail "Android Emulator acceleration check failed"
  fi
fi

avd_home="${ANDROID_AVD_HOME:-$HOME/.android/avd}"
if [[ -d "$avd_home/$ANDROID_PHONE_AVD.avd" ]]; then
  ok "pinned Android AVD exists: $ANDROID_PHONE_AVD"
else
  fail "pinned Android AVD is missing: $avd_home/$ANDROID_PHONE_AVD.avd"
fi

if (( warnings > 0 )); then
  printf '\nGuest preflight warnings: %d\n' "$warnings"
fi
if (( failures > 0 )); then
  printf 'Guest preflight failed: %d requirement(s) are not satisfied.\n' "$failures" >&2
  exit 1
fi

printf '\nOK    Unified guest is ready for local Linux and Android qualification.\n'

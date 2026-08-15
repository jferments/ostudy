#!/usr/bin/env bash
set -Eeuo pipefail

VM_NAME="${OSTUDY_VM_NAME:-ostudy-ci-unified}"
LIBVIRT_URI="${OSTUDY_LIBVIRT_URI:-qemu:///system}"
STORAGE_DIR="${OSTUDY_VM_STORAGE_DIR:-/var/lib/libvirt/images/$VM_NAME}"
RAM_GIB="${OSTUDY_VM_RAM_GIB:-64}"
DISK_GIB="${OSTUDY_VM_DISK_GIB:-150}"
SWAP_GIB="${OSTUDY_VM_SWAP_GIB:-16}"
VCPUS="${OSTUDY_VM_VCPUS:-16}"
MIN_HOST_RAM_GIB="${OSTUDY_HOST_MIN_RAM_GIB:-80}"
MIN_HOST_FREE_GIB="${OSTUDY_HOST_MIN_FREE_GIB:-170}"

failures=0
warnings=0

ok() { printf 'OK    %s\n' "$*"; }
warn() { printf 'WARN  %s\n' "$*"; warnings=$((warnings + 1)); }
fail() { printf 'FAIL  %s\n' "$*" >&2; failures=$((failures + 1)); }
info() { printf 'INFO  %s\n' "$*"; }

nearest_existing_parent() {
  local path="$1"
  while [[ ! -e "$path" && "$path" != / ]]; do
    path="$(dirname -- "$path")"
  done
  printf '%s\n' "$path"
}

command_check() {
  local command_name="$1" package_hint="$2"
  if command -v "$command_name" >/dev/null 2>&1; then
    ok "$command_name is available"
  else
    fail "$command_name is missing (install package: $package_hint)"
  fi
}

printf 'ostudy unified runner host preflight\n'
printf '  VM:       %s\n' "$VM_NAME"
printf '  URI:      %s\n' "$LIBVIRT_URI"
printf '  Storage:  %s\n' "$STORAGE_DIR"
printf '  Guest:    %s GiB RAM, %s GiB disk, %s GiB swap, %s vCPUs\n\n' \
  "$RAM_GIB" "$DISK_GIB" "$SWAP_GIB" "$VCPUS"

if [[ "$(uname -s)" == Linux && "$(uname -m)" == x86_64 ]]; then
  ok "host is Linux x86_64"
else
  fail "host must be Linux x86_64; found $(uname -s) $(uname -m)"
fi

for entry in \
  'virsh:libvirt-clients' \
  'virt-install:virtinst' \
  'qemu-img:qemu-utils' \
  'cloud-localds:cloud-image-utils' \
  'curl:curl' \
  'sha256sum:coreutils' \
  'python3:python3' \
  'ssh:ssh-client'; do
  command_check "${entry%%:*}" "${entry#*:}"
done

if command -v kvm-ok >/dev/null 2>&1; then
  if kvm-ok >/dev/null 2>&1; then
    ok "host KVM acceleration is available"
  else
    fail "kvm-ok reports that host KVM acceleration is unavailable"
  fi
else
  fail "kvm-ok is missing (install package: cpu-checker)"
fi

if [[ -e /dev/kvm ]]; then
  if [[ -r /dev/kvm && -w /dev/kvm ]]; then
    ok "/dev/kvm is readable and writable by $(id -un)"
  else
    fail "/dev/kvm exists but $(id -un) cannot read/write it; add the user to the kvm group and log out/in"
  fi
else
  fail "/dev/kvm does not exist"
fi

cpu_flag=""
if grep -qm1 -w vmx /proc/cpuinfo; then
  cpu_flag=vmx
  kvm_module=kvm_intel
elif grep -qm1 -w svm /proc/cpuinfo; then
  cpu_flag=svm
  kvm_module=kvm_amd
else
  kvm_module=""
  fail "CPU virtualization flag vmx/svm is not visible; check BIOS/UEFI virtualization settings"
fi

if [[ -n "$cpu_flag" ]]; then
  ok "CPU exposes $cpu_flag hardware virtualization"
  nested_path="/sys/module/$kvm_module/parameters/nested"
  if [[ -r "$nested_path" ]]; then
    nested_value="$(tr '[:lower:]' '[:upper:]' < "$nested_path")"
    if [[ "$nested_value" == Y || "$nested_value" == 1 ]]; then
      ok "nested virtualization is enabled for $kvm_module"
    else
      fail "nested virtualization is disabled in $nested_path (value: $nested_value)"
      info "persist with: options $kvm_module nested=1 in /etc/modprobe.d/ostudy-nested-kvm.conf"
    fi
  else
    fail "$kvm_module is not loaded or its nested parameter is unavailable"
  fi
fi

if command -v virsh >/dev/null 2>&1; then
  if virsh --connect "$LIBVIRT_URI" uri >/dev/null 2>&1; then
    ok "libvirt connection $LIBVIRT_URI is usable"
  else
    fail "cannot connect to $LIBVIRT_URI; install/start libvirt and add the user to the libvirt group"
  fi

  if virsh --connect "$LIBVIRT_URI" net-info default >/dev/null 2>&1; then
    network_state="$(virsh --connect "$LIBVIRT_URI" net-info default | awk -F: '/^Active:/ {gsub(/^[ \t]+/, "", $2); print $2}')"
    if [[ "$network_state" == yes ]]; then
      ok "libvirt default NAT network is active"
    else
      fail "libvirt default network exists but is not active (run: virsh --connect $LIBVIRT_URI net-start default)"
    fi
  else
    fail "libvirt default NAT network is missing"
  fi

  if virsh --connect "$LIBVIRT_URI" dominfo "$VM_NAME" >/dev/null 2>&1; then
    fail "a libvirt domain named $VM_NAME already exists; this script never overwrites an existing VM"
  else
    ok "no existing libvirt domain named $VM_NAME"
  fi
fi

mem_total_kib="$(awk '/^MemTotal:/ {print $2}' /proc/meminfo)"
mem_available_kib="$(awk '/^MemAvailable:/ {print $2}' /proc/meminfo)"
mem_total_gib=$((mem_total_kib / 1024 / 1024))
mem_available_gib=$((mem_available_kib / 1024 / 1024))
if (( mem_total_gib >= MIN_HOST_RAM_GIB )); then
  ok "host has ${mem_total_gib} GiB RAM (minimum configured: ${MIN_HOST_RAM_GIB} GiB)"
else
  fail "host has ${mem_total_gib} GiB RAM; at least ${MIN_HOST_RAM_GIB} GiB is required for a ${RAM_GIB} GiB guest plus host headroom"
fi
if (( mem_available_gib >= RAM_GIB )); then
  ok "host currently has ${mem_available_gib} GiB available memory"
else
  warn "host currently has ${mem_available_gib} GiB available; stop memory-heavy workloads before starting the ${RAM_GIB} GiB VM"
fi

host_cpus="$(nproc)"
if (( host_cpus >= VCPUS )); then
  ok "host exposes $host_cpus logical CPUs for a $VCPUS-vCPU guest"
else
  fail "host exposes only $host_cpus logical CPUs, fewer than requested guest vCPUs ($VCPUS)"
fi

storage_parent="$(nearest_existing_parent "$STORAGE_DIR")"
free_kib="$(df -Pk "$storage_parent" | awk 'NR == 2 {print $4}')"
free_gib=$((free_kib / 1024 / 1024))
if (( free_gib >= MIN_HOST_FREE_GIB )); then
  ok "storage filesystem has ${free_gib} GiB free (minimum configured: ${MIN_HOST_FREE_GIB} GiB)"
else
  fail "storage filesystem has ${free_gib} GiB free; at least ${MIN_HOST_FREE_GIB} GiB is required before creating the ${DISK_GIB} GiB guest disk"
fi

if [[ -e "$STORAGE_DIR" ]]; then
  fail "storage path already exists: $STORAGE_DIR"
else
  ok "storage path is unused: $STORAGE_DIR"
fi

if (( warnings > 0 )); then
  printf '\nPreflight warnings: %d\n' "$warnings"
fi
if (( failures > 0 )); then
  printf 'Preflight failed: %d requirement(s) are not satisfied.\n' "$failures" >&2
  exit 1
fi

printf '\nOK    Host is ready to create %s.\n' "$VM_NAME"

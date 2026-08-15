#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/../.." && pwd)"

VM_NAME="${OSTUDY_VM_NAME:-ostudy-ci-unified}"
LIBVIRT_URI="${OSTUDY_LIBVIRT_URI:-qemu:///system}"
STORAGE_DIR="${OSTUDY_VM_STORAGE_DIR:-/var/lib/libvirt/images/$VM_NAME}"
RAM_GIB="${OSTUDY_VM_RAM_GIB:-64}"
DISK_GIB="${OSTUDY_VM_DISK_GIB:-150}"
SWAP_GIB="${OSTUDY_VM_SWAP_GIB:-16}"
VCPUS="${OSTUDY_VM_VCPUS:-16}"
ADMIN_USER="${OSTUDY_VM_ADMIN_USER:-ostudy-admin}"
RUNNER_USER="${OSTUDY_VM_RUNNER_USER:-ostudy-runner}"
SSH_PUBLIC_KEY_PATH="${OSTUDY_VM_SSH_PUBLIC_KEY:-$HOME/.ssh/ostudy-ci-unified.pub}"
SSH_PRIVATE_KEY_PATH="${SSH_PUBLIC_KEY_PATH%.pub}"
IMAGE_BASE_URL="${OSTUDY_UBUNTU_IMAGE_BASE_URL:-https://cloud-images.ubuntu.com/releases/24.04/release}"
IMAGE_NAME="${OSTUDY_UBUNTU_IMAGE_NAME:-ubuntu-24.04-server-cloudimg-amd64.img}"
CACHE_DIR="${OSTUDY_VM_CACHE_DIR:-$HOME/.cache/ostudy-runner/images}"
WAIT_SECONDS="${OSTUDY_VM_WAIT_SECONDS:-600}"

usage() {
  cat <<'EOF'
Usage: ./scripts/runner/create-unified-runner-vm.sh

Configuration is supplied through environment variables:
  OSTUDY_VM_NAME                 default: ostudy-ci-unified
  OSTUDY_VM_RAM_GIB              default: 64
  OSTUDY_VM_DISK_GIB             default: 150
  OSTUDY_VM_SWAP_GIB             default: 16
  OSTUDY_VM_VCPUS                default: 16
  OSTUDY_VM_SSH_PUBLIC_KEY       default: ~/.ssh/ostudy-ci-unified.pub
  OSTUDY_VM_STORAGE_DIR          default: /var/lib/libvirt/images/<vm-name>
  OSTUDY_LIBVIRT_URI             default: qemu:///system

The script refuses to overwrite an existing libvirt domain or storage path.
It creates no host mounts, passthrough devices, runner registration, or secrets.
EOF
}

if [[ "${1:-}" == --help || "${1:-}" == -h ]]; then
  usage
  exit 0
fi
if (($#)); then
  usage >&2
  exit 2
fi

for command_name in sudo curl sha256sum qemu-img cloud-localds virt-install virsh python3 ssh; do
  command -v "$command_name" >/dev/null 2>&1 || {
    printf 'Missing required command: %s\n' "$command_name" >&2
    exit 1
  }
done

[[ -r "$SSH_PUBLIC_KEY_PATH" ]] || {
  printf 'SSH public key not found: %s\n' "$SSH_PUBLIC_KEY_PATH" >&2
  printf 'Create it with: ssh-keygen -t ed25519 -f %s -C ostudy-ci-unified\n' "${SSH_PUBLIC_KEY_PATH%.pub}" >&2
  exit 1
}
[[ -r "$SSH_PRIVATE_KEY_PATH" ]] || {
  printf 'Matching SSH private key not found: %s\n' "$SSH_PRIVATE_KEY_PATH" >&2
  exit 1
}

"$SCRIPT_DIR/host-preflight.sh"

if virsh --connect "$LIBVIRT_URI" dominfo "$VM_NAME" >/dev/null 2>&1; then
  printf 'Refusing to overwrite existing libvirt domain: %s\n' "$VM_NAME" >&2
  exit 1
fi
if [[ -e "$STORAGE_DIR" ]]; then
  printf 'Refusing to overwrite existing storage path: %s\n' "$STORAGE_DIR" >&2
  exit 1
fi

mkdir -p "$CACHE_DIR"
base_image="$CACHE_DIR/$IMAGE_NAME"
checksums="$CACHE_DIR/SHA256SUMS"

printf 'Downloading Ubuntu 24.04 cloud-image checksum manifest...\n'
curl --fail --location --proto '=https' --tlsv1.2 \
  "$IMAGE_BASE_URL/SHA256SUMS" \
  --output "$checksums.tmp"
mv -- "$checksums.tmp" "$checksums"

expected_sha="$(awk -v name="$IMAGE_NAME" '
  {
    file=$2
    sub(/^\*/, "", file)
    if (file == name) { print $1; exit }
  }
' "$checksums")"
[[ "$expected_sha" =~ ^[0-9a-fA-F]{64}$ ]] || {
  printf 'Could not find a valid SHA-256 for %s in %s/SHA256SUMS.\n' "$IMAGE_NAME" "$IMAGE_BASE_URL" >&2
  exit 1
}

if [[ ! -f "$base_image" ]] || [[ "$(sha256sum "$base_image" | awk '{print $1}')" != "$expected_sha" ]]; then
  printf 'Downloading verified Ubuntu cloud image %s...\n' "$IMAGE_NAME"
  curl --fail --location --proto '=https' --tlsv1.2 \
    "$IMAGE_BASE_URL/$IMAGE_NAME" \
    --output "$base_image.partial"
  actual_sha="$(sha256sum "$base_image.partial" | awk '{print $1}')"
  [[ "$actual_sha" == "$expected_sha" ]] || {
    rm -f -- "$base_image.partial"
    printf 'Ubuntu cloud image SHA-256 mismatch. Expected %s, got %s.\n' "$expected_sha" "$actual_sha" >&2
    exit 1
  }
  mv -- "$base_image.partial" "$base_image"
else
  printf 'Reusing verified Ubuntu cloud image %s.\n' "$base_image"
fi

work_dir="$(mktemp -d -t ostudy-unified-vm.XXXXXXXX)"
cleanup() { rm -rf -- "$work_dir"; }
trap cleanup EXIT

vm_disk="$work_dir/$VM_NAME.qcow2"
seed_iso="$work_dir/$VM_NAME-seed.iso"
user_data="$work_dir/user-data"
meta_data="$work_dir/meta-data"

printf 'Creating independent %s GiB qcow2 disk...\n' "$DISK_GIB"
qemu-img convert -p -O qcow2 "$base_image" "$vm_disk"
qemu-img resize "$vm_disk" "${DISK_GIB}G"
qemu-img info "$vm_disk"

ssh_key="$(tr -d '\r\n' < "$SSH_PUBLIC_KEY_PATH")"
ssh_key_yaml="$(python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))' <<< "$ssh_key")"

cat > "$user_data" <<EOF
#cloud-config
hostname: $VM_NAME
fqdn: $VM_NAME.local
manage_etc_hosts: true
disable_root: true
ssh_pwauth: false
users:
  - name: $ADMIN_USER
    gecos: ostudy runner administrator
    groups: [adm, sudo]
    shell: /bin/bash
    lock_passwd: true
    sudo: ["ALL=(ALL) NOPASSWD:ALL"]
    ssh_authorized_keys:
      - $ssh_key_yaml
  - name: $RUNNER_USER
    gecos: ostudy GitHub Actions runner
    groups: [kvm]
    shell: /bin/bash
    lock_passwd: true
packages:
  - ca-certificates
  - curl
  - git
  - jq
  - openssh-server
  - qemu-guest-agent
  - rsync
  - ufw
  - unzip
  - xz-utils
  - zip
package_update: true
package_upgrade: false
growpart:
  mode: auto
  devices: ['/']
resize_rootfs: true
write_files:
  - path: /etc/ssh/sshd_config.d/90-ostudy-runner.conf
    owner: root:root
    permissions: '0644'
    content: |
      PasswordAuthentication no
      KbdInteractiveAuthentication no
      PermitRootLogin no
      AllowUsers $ADMIN_USER
  - path: /etc/sysctl.d/90-ostudy-runner.conf
    owner: root:root
    permissions: '0644'
    content: |
      kernel.dmesg_restrict=1
      kernel.kptr_restrict=2
runcmd:
  - [systemctl, enable, --now, qemu-guest-agent]
  - [systemctl, restart, ssh]
  - [bash, -c, "if ! swapon --show=NAME --noheadings | grep -Fxq /swapfile; then rm -f /swapfile; fallocate -l ${SWAP_GIB}G /swapfile; chmod 600 /swapfile; mkswap /swapfile; swapon /swapfile; fi"]
  - [bash, -c, "grep -q '^/swapfile ' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab"]
  - [ufw, default, deny, incoming]
  - [ufw, default, allow, outgoing]
  - [ufw, allow, OpenSSH]
  - [ufw, --force, enable]
  - [sysctl, --system]
  - [bash, -c, "install -d -o $RUNNER_USER -g $RUNNER_USER -m 0750 /home/$RUNNER_USER/actions-runner /home/$RUNNER_USER/.cache/ostudy"]
  - [bash, -c, "touch /var/lib/cloud/instance/ostudy-runner-cloud-init-complete"]
final_message: "ostudy unified runner base VM cloud-init completed"
EOF

cat > "$meta_data" <<EOF
instance-id: $VM_NAME-$(date -u +%Y%m%d%H%M%S)
local-hostname: $VM_NAME
EOF

cloud-localds "$seed_iso" "$user_data" "$meta_data"

printf 'Installing VM files under %s...\n' "$STORAGE_DIR"
sudo install -d -o libvirt-qemu -g kvm -m 0750 "$STORAGE_DIR"
sudo install -o libvirt-qemu -g kvm -m 0640 "$vm_disk" "$STORAGE_DIR/$VM_NAME.qcow2"
sudo install -o libvirt-qemu -g kvm -m 0640 "$seed_iso" "$STORAGE_DIR/$VM_NAME-seed.iso"
printf '%s  %s\n' "$expected_sha" "$IMAGE_NAME" | sudo tee "$STORAGE_DIR/BASE_IMAGE_SHA256" >/dev/null
sudo chmod 0640 "$STORAGE_DIR/BASE_IMAGE_SHA256"
sudo chown libvirt-qemu:kvm "$STORAGE_DIR/BASE_IMAGE_SHA256"

printf 'Defining and starting %s...\n' "$VM_NAME"
virt-install \
  --connect "$LIBVIRT_URI" \
  --name "$VM_NAME" \
  --description 'ostudy unified Linux and Android CI runner' \
  --virt-type kvm \
  --machine q35 \
  --cpu host-passthrough \
  --vcpus "$VCPUS" \
  --memory $((RAM_GIB * 1024)) \
  --osinfo ubuntu24.04 \
  --import \
  --boot uefi \
  --disk "path=$STORAGE_DIR/$VM_NAME.qcow2,format=qcow2,bus=virtio,cache=none,discard=unmap" \
  --disk "path=$STORAGE_DIR/$VM_NAME-seed.iso,device=cdrom,readonly=on" \
  --network network=default,model=virtio \
  --channel unix,target_type=virtio,name=org.qemu.guest_agent.0 \
  --graphics none \
  --console pty,target_type=serial \
  --noautoconsole

virsh --connect "$LIBVIRT_URI" autostart "$VM_NAME"

xml="$(virsh --connect "$LIBVIRT_URI" dumpxml "$VM_NAME")"
grep -Fq "<cpu mode='host-passthrough'" <<< "$xml" || {
  printf 'Defined VM does not use host-passthrough CPU mode.\n' >&2
  exit 1
}
if grep -Eq '<filesystem|<hostdev' <<< "$xml"; then
  printf 'Defined VM unexpectedly contains a host filesystem or passthrough device.\n' >&2
  exit 1
fi
printf 'OK   VM XML uses host-passthrough and contains no host shares/passthrough devices.\n'

printf 'Waiting up to %s seconds for a guest IP...\n' "$WAIT_SECONDS"
deadline=$((SECONDS + WAIT_SECONDS))
guest_ip=""
while ((SECONDS < deadline)); do
  guest_ip="$({
    virsh --connect "$LIBVIRT_URI" domifaddr "$VM_NAME" --source agent 2>/dev/null || true
    virsh --connect "$LIBVIRT_URI" domifaddr "$VM_NAME" --source lease 2>/dev/null || true
  } | awk '/ipv4/ {split($4, parts, "/"); print parts[1]; exit}')"
  [[ -n "$guest_ip" ]] && break
  sleep 2
done
[[ -n "$guest_ip" ]] || {
  printf 'VM was created, but no IP was detected. Inspect with:\n' >&2
  printf '  virsh --connect %s console %s\n' "$LIBVIRT_URI" "$VM_NAME" >&2
  printf '  virsh --connect %s domifaddr %s\n' "$LIBVIRT_URI" "$VM_NAME" >&2
  exit 1
}

printf 'Guest IP: %s\n' "$guest_ip"
printf 'Waiting for SSH and cloud-init completion...\n'
while ((SECONDS < deadline)); do
  if ssh \
    -i "$SSH_PRIVATE_KEY_PATH" \
    -o BatchMode=yes \
    -o ConnectTimeout=5 \
    -o StrictHostKeyChecking=accept-new \
    "$ADMIN_USER@$guest_ip" \
    'cloud-init status --wait && test -f /var/lib/cloud/instance/ostudy-runner-cloud-init-complete' >/dev/null 2>&1; then
    break
  fi
  sleep 3
done

if ! ssh \
  -i "$SSH_PRIVATE_KEY_PATH" \
  -o BatchMode=yes \
  -o ConnectTimeout=5 \
  -o StrictHostKeyChecking=accept-new \
  "$ADMIN_USER@$guest_ip" \
  'cloud-init status --wait && test -f /var/lib/cloud/instance/ostudy-runner-cloud-init-complete'; then
  printf 'VM is running, but cloud-init/SSH did not complete within the deadline.\n' >&2
  exit 1
fi

printf '\nOK   Created isolated unified runner VM.\n'
printf 'Connect with:\n  ssh -i %q %q\n' "$SSH_PRIVATE_KEY_PATH" "$ADMIN_USER@$guest_ip"
printf 'Next: copy this branch into the guest and run scripts/runner/bootstrap-unified-guest.sh.\n'
printf 'Do not register the GitHub runner until guest-preflight.sh and local Linux/Android qualification pass.\n'

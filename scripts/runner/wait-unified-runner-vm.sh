#!/usr/bin/env bash
set -Eeuo pipefail

VM_NAME="${OSTUDY_VM_NAME:-ostudy-ci-unified}"
LIBVIRT_URI="${OSTUDY_LIBVIRT_URI:-qemu:///system}"
ADMIN_USER="${OSTUDY_VM_ADMIN_USER:-ostudy-admin}"
SSH_PUBLIC_KEY_PATH="${OSTUDY_VM_SSH_PUBLIC_KEY:-$HOME/.ssh/ostudy-ci-unified.pub}"
SSH_PRIVATE_KEY_PATH="${SSH_PUBLIC_KEY_PATH%.pub}"
GUEST_IP="${OSTUDY_VM_GUEST_IP:-}"
WAIT_SECONDS="${OSTUDY_VM_WAIT_SECONDS:-600}"
MARKER="/var/lib/cloud/instance/ostudy-runner-cloud-init-complete"

usage() {
  cat <<'EOF'
Usage: ./scripts/runner/wait-unified-runner-vm.sh

Wait for an already-defined ostudy unified VM to become reachable and finish
cloud-init. This script never creates, deletes, resets, or modifies the VM.

Environment:
  OSTUDY_VM_NAME            default: ostudy-ci-unified
  OSTUDY_LIBVIRT_URI        default: qemu:///system
  OSTUDY_VM_ADMIN_USER      default: ostudy-admin
  OSTUDY_VM_SSH_PUBLIC_KEY  default: ~/.ssh/ostudy-ci-unified.pub
  OSTUDY_VM_GUEST_IP        optional known guest IP
  OSTUDY_VM_WAIT_SECONDS    default: 600

Passphrase-protected administration keys must already be loaded into the local
ssh-agent. For example:

  ssh-add -t 4h ~/.ssh/ostudy-ci-unified
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

for command_name in virsh ssh ssh-keygen ssh-add awk grep; do
  command -v "$command_name" >/dev/null 2>&1 || {
    printf 'Missing required command: %s\n' "$command_name" >&2
    exit 1
  }
done

[[ -r "$SSH_PUBLIC_KEY_PATH" ]] || {
  printf 'SSH public key not found: %s\n' "$SSH_PUBLIC_KEY_PATH" >&2
  exit 1
}
[[ -r "$SSH_PRIVATE_KEY_PATH" ]] || {
  printf 'SSH private key not found: %s\n' "$SSH_PRIVATE_KEY_PATH" >&2
  exit 1
}

# BatchMode is intentional so readiness probes can never hang on a passphrase
# prompt. An encrypted key therefore has to be unlocked into the host agent.
if ! ssh-keygen -y -P '' -f "$SSH_PRIVATE_KEY_PATH" >/dev/null 2>&1; then
  public_body="$(awk 'NF >= 2 {print $2; exit}' "$SSH_PUBLIC_KEY_PATH")"
  if [[ -z "$public_body" ]] \
    || ! ssh-add -L 2>/dev/null | awk 'NF >= 2 {print $2}' | grep -Fxq "$public_body"; then
    printf 'The SSH administration key is passphrase-protected and is not loaded in ssh-agent.\n' >&2
    printf 'Load it once, then rerun this command:\n' >&2
    printf '  ssh-add -t 4h %q\n' "$SSH_PRIVATE_KEY_PATH" >&2
    exit 1
  fi
  printf 'OK   passphrase-protected administration key is loaded in ssh-agent.\n'
else
  printf 'OK   administration key can be used non-interactively.\n'
fi

virsh --connect "$LIBVIRT_URI" dominfo "$VM_NAME" >/dev/null 2>&1 || {
  printf 'Libvirt domain does not exist: %s\n' "$VM_NAME" >&2
  exit 1
}

state="$(virsh --connect "$LIBVIRT_URI" domstate "$VM_NAME" | tr -d '\r')"
[[ "$state" == running ]] || {
  printf 'VM %s is not running (state: %s). Start it before waiting for readiness.\n' "$VM_NAME" "$state" >&2
  exit 1
}

ssh_opts=(
  -i "$SSH_PRIVATE_KEY_PATH"
  -o BatchMode=yes
  -o ConnectTimeout=5
  -o StrictHostKeyChecking=accept-new
)

deadline=$((SECONDS + WAIT_SECONDS))

if [[ -z "$GUEST_IP" ]]; then
  printf 'Waiting up to %s seconds for a guest IP...\n' "$WAIT_SECONDS"
  while ((SECONDS < deadline)); do
    GUEST_IP="$({
      virsh --connect "$LIBVIRT_URI" domifaddr "$VM_NAME" --source agent 2>/dev/null || true
      virsh --connect "$LIBVIRT_URI" domifaddr "$VM_NAME" --source lease 2>/dev/null || true
    } | awk '/ipv4/ {split($4, parts, "/"); print parts[1]; exit}')"
    [[ -n "$GUEST_IP" ]] && break
    sleep 2
  done
fi

[[ -n "$GUEST_IP" ]] || {
  printf 'No guest IPv4 address was detected for %s.\n' "$VM_NAME" >&2
  exit 1
}
printf 'Guest IP: %s\n' "$GUEST_IP"

printf 'Waiting for SSH and cloud-init completion...\n'
last_report=0
while ((SECONDS < deadline)); do
  if probe="$({
    ssh "${ssh_opts[@]}" "$ADMIN_USER@$GUEST_IP" \
      'status="$(cloud-init status 2>&1 || true)"; printf "%s\n" "$status"; test -f /var/lib/cloud/instance/ostudy-runner-cloud-init-complete && printf "__OSTUDY_MARKER__\n"' \
      2>/dev/null || true
  })"; then
    :
  fi

  if grep -Fq '__OSTUDY_MARKER__' <<< "$probe"; then
    printf 'OK   cloud-init completion marker is present.\n'
    break
  fi

  if grep -Eq '^status:[[:space:]]+error' <<< "$probe"; then
    printf 'cloud-init reports an error before the completion marker was created.\n' >&2
    ssh "${ssh_opts[@]}" "$ADMIN_USER@$GUEST_IP" \
      'cloud-init status --long || true; sudo tail -n 160 /var/log/cloud-init-output.log || true' >&2 || true
    exit 1
  fi

  if ((SECONDS - last_report >= 15)); then
    status_line="$(grep -E '^status:' <<< "$probe" | tail -n 1 || true)"
    printf '  still waiting%s\n' "${status_line:+ ($status_line)}"
    last_report=$SECONDS
  fi
  sleep 3
done

if ! ssh "${ssh_opts[@]}" "$ADMIN_USER@$GUEST_IP" "test -f '$MARKER'"; then
  printf 'VM is reachable, but cloud-init did not create %s within %s seconds.\n' "$MARKER" "$WAIT_SECONDS" >&2
  ssh "${ssh_opts[@]}" "$ADMIN_USER@$GUEST_IP" \
    'cloud-init status --long || true; sudo systemctl --failed --no-pager || true; sudo tail -n 160 /var/log/cloud-init-output.log || true' >&2 || true
  exit 1
fi

# cloud-init exits 2 for a completed run with recoverable errors. Show those
# details, but do not treat them as a provisioning failure when our explicit
# final marker exists. Exit 1 remains a hard error.
set +e
ssh "${ssh_opts[@]}" "$ADMIN_USER@$GUEST_IP" '
  set +e
  cloud-init status --long
  rc=$?
  set -e
  case "$rc" in
    0|2) exit 0 ;;
    *) exit "$rc" ;;
  esac
'
status_rc=$?
set -e
if ((status_rc != 0)); then
  printf 'cloud-init completed its marker path but reports a hard error. Inspect the guest logs before continuing.\n' >&2
  exit "$status_rc"
fi

printf '\nOK   %s is reachable and cloud-init provisioning is complete.\n' "$VM_NAME"
printf 'Guest IP: %s\n' "$GUEST_IP"
printf 'Connect with:\n  ssh -i %q %q\n' "$SSH_PRIVATE_KEY_PATH" "$ADMIN_USER@$GUEST_IP"

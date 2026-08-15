# Set up the unified Linux and Android runner on ML-tower

This guide provisions `ostudy-ci-unified`, one Ubuntu 24.04 LTS VM that runs both Linux/Tauri and Android/KVM qualification. It does not replace or delete existing runners during setup.

Do not paste GitHub runner registration tokens into documentation, chat, shared logs, or committed files. Use the current one-time token shown by GitHub only inside the guest.

## Checkpoint 0: update the infrastructure branch

On `ML-tower`:

```bash
cd ~/code/ostudy

git fetch origin --prune
git switch --track origin/agent/unified-linux-android-runner \
  || git switch agent/unified-linux-android-runner
git pull --ff-only

git rev-parse --show-toplevel
git rev-parse HEAD
```

The top level must be `/home/jesse/code/ostudy`. Do not create another nested clone.

## Checkpoint 1: install host virtualization tools

```bash
sudo apt update
sudo apt install -y \
  cloud-image-utils \
  cpu-checker \
  libvirt-clients \
  libvirt-daemon-system \
  ovmf \
  qemu-kvm \
  qemu-utils \
  virtinst

sudo systemctl enable --now libvirtd.service
sudo usermod -aG libvirt,kvm "$USER"
```

Log out of the desktop session and back in so the new groups apply. A new terminal alone is not sufficient.

Verify:

```bash
id
kvm-ok
virsh --connect qemu:///system uri
```

Expected:

- `id` includes `libvirt` and `kvm`;
- `kvm-ok` reports that KVM acceleration can be used;
- `virsh` prints `qemu:///system`.

## Checkpoint 2: verify nested KVM on the host

Detect the CPU virtualization module:

```bash
if grep -qm1 -w vmx /proc/cpuinfo; then
  KVM_MODULE=kvm_intel
elif grep -qm1 -w svm /proc/cpuinfo; then
  KVM_MODULE=kvm_amd
else
  echo "No vmx/svm virtualization flag is visible" >&2
  exit 1
fi

printf 'KVM module: %s\n' "$KVM_MODULE"
cat "/sys/module/$KVM_MODULE/parameters/nested"
```

If the value is `Y` or `1`, nested KVM is already enabled.

If it is disabled, persist the setting and reboot. Do **not** unload KVM modules while Multipass, libvirt, Android emulators, or another VM is running.

```bash
echo "options $KVM_MODULE nested=1" \
  | sudo tee /etc/modprobe.d/ostudy-nested-kvm.conf
sudo reboot
```

After reboot, repeat the check and require `Y` or `1`.

## Checkpoint 3: activate libvirt's private NAT network

```bash
virsh --connect qemu:///system net-info default \
  || virsh --connect qemu:///system net-define /usr/share/libvirt/networks/default.xml

virsh --connect qemu:///system net-start default 2>/dev/null || true
virsh --connect qemu:///system net-autostart default
virsh --connect qemu:///system net-info default
```

The network must report `Active: yes` and `Autostart: yes`. The VM will use this NAT network; do not create a bridge or port forward.

## Checkpoint 4: create a dedicated administration SSH key

```bash
ssh-keygen \
  -t ed25519 \
  -f ~/.ssh/ostudy-ci-unified \
  -C ostudy-ci-unified

chmod 600 ~/.ssh/ostudy-ci-unified
chmod 644 ~/.ssh/ostudy-ci-unified.pub
```

This key is only for administering the runner VM. It is not copied to the GitHub runner account and is not forwarded into CI jobs.

## Checkpoint 5: run the read-only host preflight

```bash
cd ~/code/ostudy

OSTUDY_VM_RAM_GIB=64 \
OSTUDY_VM_DISK_GIB=150 \
OSTUDY_VM_SWAP_GIB=16 \
OSTUDY_VM_VCPUS=16 \
OSTUDY_VM_SSH_PUBLIC_KEY="$HOME/.ssh/ostudy-ci-unified.pub" \
  ./scripts/runner/host-preflight.sh
```

Do not proceed unless every requirement passes. The preflight checks the host architecture, KVM, nested KVM, libvirt, NAT network, CPU count, memory, storage, and absence of an existing VM/storage path.

The default storage safety threshold is 170 GiB free even though the requested virtual disk is 150 GiB. Override only after deliberately reviewing host capacity:

```bash
OSTUDY_HOST_MIN_FREE_GIB=160 ./scripts/runner/host-preflight.sh
```

## Checkpoint 6: create the VM

```bash
cd ~/code/ostudy

OSTUDY_VM_RAM_GIB=64 \
OSTUDY_VM_DISK_GIB=150 \
OSTUDY_VM_SWAP_GIB=16 \
OSTUDY_VM_VCPUS=16 \
OSTUDY_VM_SSH_PUBLIC_KEY="$HOME/.ssh/ostudy-ci-unified.pub" \
  ./scripts/runner/create-unified-runner-vm.sh
```

The script:

- downloads the Ubuntu 24.04 release cloud image and verifies it against the release SHA-256 manifest;
- creates an independent 150 GiB qcow2 disk;
- creates `ostudy-admin` and an unprivileged `ostudy-runner` account;
- creates and persists 16 GiB swap;
- enables SSH key authentication and disables password/root SSH login;
- enables UFW with default-deny inbound policy;
- defines the guest with 64 GiB RAM, 16 vCPUs, virtio devices, UEFI, the default NAT network, and `host-passthrough` CPU mode;
- defines no host filesystem or device passthrough;
- enables VM autostart;
- waits for cloud-init and prints the guest IP/SSH command.

Verify the definition from the host:

```bash
virsh --connect qemu:///system dominfo ostudy-ci-unified
virsh --connect qemu:///system domifaddr ostudy-ci-unified
virsh --connect qemu:///system dumpxml ostudy-ci-unified \
  | grep -E "host-passthrough|filesystem|hostdev|network='default'"
```

Expected:

- 64 GiB maximum memory;
- 16 vCPUs;
- `host-passthrough` CPU;
- `network='default'`;
- no `<filesystem>` or `<hostdev>` entries.

## Checkpoint 7: transfer a Git bundle into the isolated guest

Set the guest IP reported by the creation script:

```bash
GUEST_IP=<guest-ip>
```

Create a Git bundle containing the published infrastructure branch. A bundle preserves the real Git history needed by repository policy and diff-aware scripts without copying GitHub credentials or mounting the host checkout.

```bash
cd ~/code/ostudy

git fetch origin --prune
git switch agent/unified-linux-android-runner
git pull --ff-only

git bundle create /tmp/ostudy-unified-runner.bundle \
  agent/unified-linux-android-runner

scp -i ~/.ssh/ostudy-ci-unified \
  /tmp/ostudy-unified-runner.bundle \
  ostudy-admin@"$GUEST_IP":/tmp/

ssh -i ~/.ssh/ostudy-ci-unified \
  ostudy-admin@"$GUEST_IP" \
  'rm -rf /tmp/ostudy-runner-infra && \
   git clone /tmp/ostudy-unified-runner.bundle /tmp/ostudy-runner-infra && \
   git -C /tmp/ostudy-runner-infra switch agent/unified-linux-android-runner && \
   git -C /tmp/ostudy-runner-infra rev-parse HEAD'
```

The reported guest commit must match the host branch head. This is a one-time SSH transfer, not a persistent host mount.

## Checkpoint 8: bootstrap Linux, Rust, and Android tooling

```bash
ssh -t -i ~/.ssh/ostudy-ci-unified \
  ostudy-admin@"$GUEST_IP" \
  'cd /tmp/ostudy-runner-infra && ./scripts/runner/bootstrap-unified-guest.sh'
```

This installs the repository-pinned toolchains, creates the pinned Android AVD under the unprivileged runner account, and preserves Git metadata in `/opt/ostudy-runner-bootstrap` for local qualification. It does **not** download or register the GitHub Actions runner.

## Checkpoint 9: verify resources and nested Android acceleration inside the VM

```bash
ssh -t -i ~/.ssh/ostudy-ci-unified \
  ostudy-admin@"$GUEST_IP" \
  'sudo -u ostudy-runner -H bash -lc "cd /opt/ostudy-runner-bootstrap && ./scripts/runner/guest-preflight.sh"'
```

The guest preflight must confirm:

- approximately 64 GiB RAM;
- approximately 150 GiB root disk;
- approximately 16 GiB swap;
- 16 vCPUs;
- `vmx` or `svm` visible inside the guest;
- `/dev/kvm` usable by `ostudy-runner`;
- `kvm-ok` success;
- Android Emulator acceleration success;
- exact Node/npm/pnpm/Rust/Cargo/JDK versions;
- all Android Rust targets;
- pinned SDK, Build Tools, NDK, emulator image, and AVD;
- no host/network filesystem mounts;
- no Docker or libvirt host-control socket.

Stop here and report the output before runner registration if any check fails.

## Checkpoint 10: run the unchanged known-green commands locally

Enter a login shell as the runner user:

```bash
ssh -t -i ~/.ssh/ostudy-ci-unified \
  ostudy-admin@"$GUEST_IP" \
  'sudo -u ostudy-runner -H bash -l'
```

Inside the VM:

```bash
cd /opt/ostudy-runner-bootstrap

./scripts/check-fast.sh
./scripts/check.sh
./scripts/android-test.sh
./scripts/android-emulator-smoke.sh --artifacts "$HOME/android-smoke-pre-registration"
```

These are the existing authoritative scripts from the known-green baseline. The infrastructure branch must not rewrite their behavior to make provisioning pass.

Exit the runner shell after all commands pass.

## Checkpoint 11: remove bootstrap passwordless administration

Cloud-init temporarily gives `ostudy-admin` passwordless sudo so the bootstrap can run unattended. Before creating the clean snapshot or registering GitHub, assign a strong local administrator password and remove the temporary `NOPASSWD` rule.

Connect as `ostudy-admin`:

```bash
ssh -t -i ~/.ssh/ostudy-ci-unified \
  ostudy-admin@"$GUEST_IP"
```

Inside the VM:

```bash
sudo passwd ostudy-admin

mapfile -t sudoer_files < <(
  sudo grep -RIl \
    '^ostudy-admin[[:space:]].*NOPASSWD:ALL' \
    /etc/sudoers.d 2>/dev/null
)

for sudoer_file in "${sudoer_files[@]}"; do
  sudo sed -i \
    '/^ostudy-admin[[:space:]].*NOPASSWD:ALL/d' \
    "$sudoer_file"
done

sudo visudo -cf /etc/sudoers
sudo -k
sudo true
```

The final `sudo true` must prompt for the password you just assigned. The `ostudy-runner` account remains locked for direct login and has no sudo permission.

## Checkpoint 12: create the pre-registration snapshot

The Android emulator must be stopped before snapshotting. Exit the guest, then on `ML-tower`:

```bash
virsh --connect qemu:///system shutdown ostudy-ci-unified

until [[ "$(virsh --connect qemu:///system domstate ostudy-ci-unified)" == "shut off" ]]; do
  sleep 2
done

virsh --connect qemu:///system snapshot-create-as \
  --domain ostudy-ci-unified \
  --name pre-github-runner \
  --description 'Pinned Linux/Android toolchains; no GitHub runner registration' \
  --disk-only \
  --atomic

virsh --connect qemu:///system start ostudy-ci-unified
```

If snapshot creation fails, do not ignore it. Record the error and use a powered-off qcow2 backup before proceeding.

After restart, refresh the guest IP:

```bash
virsh --connect qemu:///system domifaddr ostudy-ci-unified
```

## Checkpoint 13: register the repository runner

In GitHub, open **Settings → Actions → Runners → New self-hosted runner → Linux → x64**. Use the current download and extraction commands shown by GitHub.

Do not reuse a token from an old runner. Do not paste the token into chat.

Connect to the guest and become the runner user:

```bash
GUEST_IP=<current-guest-ip>

ssh -t -i ~/.ssh/ostudy-ci-unified \
  ostudy-admin@"$GUEST_IP" \
  'sudo -u ostudy-runner -H bash -l'
```

Inside the VM:

```bash
cd ~/actions-runner
```

Run GitHub's current download/extract commands. Before configuration, explicitly load the pinned non-interactive service environment:

```bash
set -a
source /etc/ostudy-runner.env
set +a
```

Then register:

```bash
./config.sh \
  --url https://github.com/jferments/ostudy \
  --token <ONE-TIME-TOKEN-FROM-GITHUB> \
  --name ostudy-ci-unified \
  --labels ostudy-unified \
  --work _work \
  --unattended
```

Verify that configuration captured the toolchain environment before installing the service:

```bash
grep -E '^(JAVA_HOME|ANDROID_HOME|ANDROID_SDK_ROOT)=' .env
cat .path
```

`.path` must include the JDK, Android SDK tools, Cargo, and `/usr/local/bin` paths from `/etc/ostudy-runner.env`.

Exit back to `ostudy-admin`, then install the service under the runner account. Sudo now requires the administrator password configured at Checkpoint 11.

```bash
cd /home/ostudy-runner/actions-runner
sudo ./svc.sh install ostudy-runner
sudo ./svc.sh start
sudo ./svc.sh status
```

On Ubuntu systems with `needrestart`:

```bash
sudo mkdir -p /etc/needrestart/conf.d

echo '$nrconf{override_rc}{qr(^actions\.runner\..+\.service$)} = 0;' \
  | sudo tee /etc/needrestart/conf.d/actions_runner_services.conf
```

GitHub should show `ostudy-ci-unified` as **Online / Idle** with:

```text
self-hosted
Linux
X64
ostudy-unified
```

Do not add `ostudy`, `android`, or `kvm` yet. The unique label prevents ordinary workflows from moving to this VM before shadow qualification.

## Checkpoint 14: hand control back for shadow qualification

Report that the runner is online. The PR body will then receive exactly one marker:

```text
<!-- unified-runner:preflight -->
```

That marker activates the PR's shadow workflow on the new runner without changing existing production workflow routing. The stages will progress deliberately:

```text
preflight → linux-fast → linux-full → android-full → all-full
```

Existing runners remain registered and ordinary workflows remain unchanged until all shadow stages pass.

## Do not remove old runners yet

Old runner retirement is the final cutover stage. Until then:

- leave the old Multipass VM and runner registration intact;
- leave the temporary bare-metal Android runner intact but idle when not needed;
- do not delete runner directories or systemd units;
- do not change current required-check settings.

After the unified VM passes exact-head Linux and Android qualification, a separate reviewed step will stop, unregister, and remove the old runners with rollback instructions.

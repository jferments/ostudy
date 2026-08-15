# Set up and operate the unified Linux and Android runner

This guide provisions or rebuilds `ostudy-ci-unified`, the isolated Ubuntu 24.04 VM used for Linux/Tauri and Android CI.

Do not paste GitHub runner registration/removal tokens into chat, documentation, commits, or shared logs.

## 1. Host prerequisites

On `ML-tower`, install and enable the virtualization stack:

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
```

The host user must have `libvirt` and `kvm` membership. After adding those groups, log out of the desktop session and back in before relying on them.

Verify:

```bash
id
kvm-ok
virsh --connect qemu:///system uri
```

## 2. Verify nested KVM

Detect the host virtualization module:

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

Require `Y` or `1`. If nesting must be enabled, persist the module option and reboot rather than unloading KVM while other VMs/emulators are active.

## 3. Verify private libvirt NAT

```bash
virsh --connect qemu:///system net-info default \
  || virsh --connect qemu:///system net-define /usr/share/libvirt/networks/default.xml

virsh --connect qemu:///system net-start default 2>/dev/null || true
virsh --connect qemu:///system net-autostart default
virsh --connect qemu:///system net-info default
```

The VM uses this NAT network only. Do not add a bridge, host filesystem share, or inbound port forward.

## 4. Administration SSH key

Use a dedicated passphrase-protected administration key:

```bash
ssh-keygen \
  -t ed25519 \
  -f ~/.ssh/ostudy-ci-unified \
  -C ostudy-ci-unified

chmod 600 ~/.ssh/ostudy-ci-unified
chmod 644 ~/.ssh/ostudy-ci-unified.pub
```

The private key remains on `ML-tower` and is never copied/forwarded into the runner account.

## 5. Host preflight and VM creation

From the canonical checkout:

```bash
cd ~/code/ostudy

OSTUDY_VM_RAM_GIB=64 \
OSTUDY_VM_DISK_GIB=150 \
OSTUDY_VM_SWAP_GIB=16 \
OSTUDY_VM_VCPUS=16 \
OSTUDY_VM_SSH_PUBLIC_KEY="$HOME/.ssh/ostudy-ci-unified.pub" \
  ./scripts/runner/host-preflight.sh
```

Do not continue if a required preflight fails.

Create the VM:

```bash
OSTUDY_VM_RAM_GIB=64 \
OSTUDY_VM_DISK_GIB=150 \
OSTUDY_VM_SWAP_GIB=16 \
OSTUDY_VM_VCPUS=16 \
OSTUDY_VM_SSH_PUBLIC_KEY="$HOME/.ssh/ostudy-ci-unified.pub" \
  ./scripts/runner/create-unified-runner-vm.sh
```

The creator defines independent qcow2 storage, UEFI, `host-passthrough`, the private NAT interface, no host filesystem/device passthrough, `ostudy-admin`, and the unprivileged `ostudy-runner` account.

Verify before bootstrap:

```bash
virsh --connect qemu:///system dominfo ostudy-ci-unified
virsh --connect qemu:///system domifaddr ostudy-ci-unified
virsh --connect qemu:///system dumpxml ostudy-ci-unified \
  | grep -E "host-passthrough|filesystem|hostdev|network='default'"
```

There must be no `<filesystem>` or `<hostdev>` entries.

## 6. Transfer source without host mounts or GitHub credentials

Set the current guest IP from `virsh domifaddr`:

```bash
GUEST_IP=<current-guest-ip>
```

Create a Git bundle from the published branch being qualified:

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
```

Clone the bundle inside the guest and verify the exact commit:

```bash
ssh -i ~/.ssh/ostudy-ci-unified \
  ostudy-admin@"$GUEST_IP" \
  'rm -rf /tmp/ostudy-runner-infra && \
   git clone /tmp/ostudy-unified-runner.bundle /tmp/ostudy-runner-infra && \
   git -C /tmp/ostudy-runner-infra switch agent/unified-linux-android-runner && \
   git -C /tmp/ostudy-runner-infra rev-parse HEAD'
```

The guest SHA must equal the published branch SHA.

## 7. Bootstrap the guest

```bash
ssh -t -i ~/.ssh/ostudy-ci-unified \
  ostudy-admin@"$GUEST_IP" \
  'cd /tmp/ostudy-runner-infra && ./scripts/runner/bootstrap-unified-guest.sh'
```

The bootstrap installs the pinned Linux/Tauri/Android toolchains, configures persistent caches and the pinned AVD, and places the qualified repository tree at `/opt/ostudy-runner-bootstrap`. It does not register GitHub.

## 8. Guest preflight and local qualification

Run preflight as the unprivileged runner:

```bash
ssh -t -i ~/.ssh/ostudy-ci-unified \
  ostudy-admin@"$GUEST_IP" \
  'sudo -u ostudy-runner -H bash -lc "
    set -a
    source /etc/ostudy-runner.env
    set +a
    cd /opt/ostudy-runner-bootstrap
    ./scripts/runner/guest-preflight.sh
  "'
```

An unprivileged warning that UFW cannot be inspected is acceptable only if UFW is separately verified as `ostudy-admin`.

For local qualification, first open an ordinary admin SSH session:

```bash
ssh -t -i ~/.ssh/ostudy-ci-unified \
  ostudy-admin@"$GUEST_IP"
```

From the **interactive admin shell**, enter a nested runner shell:

```bash
sudo -u ostudy-runner -H bash -l
```

Now the prompt should be `ostudy-runner@ostudy-ci-unified`. Run:

```bash
cd /opt/ostudy-runner-bootstrap

./scripts/check-fast.sh
./scripts/check.sh
./scripts/android-test.sh
./scripts/android-emulator-smoke.sh \
  --artifacts "$HOME/android-smoke-pre-registration"
```

When these finish, `exit` returns to the already-open `ostudy-admin` shell because the runner shell was started *inside* that interactive SSH session.

Do not use a one-shot SSH command that directly launches the runner shell if you expect `exit` to return to `ostudy-admin`; in that form, exiting the runner shell ends the SSH session and returns to `ML-tower`.

## 9. Harden administration before GitHub registration

From the `ostudy-admin` shell, set a strong local administrator password:

```bash
sudo passwd ostudy-admin
```

Find and remove only the temporary bootstrap `NOPASSWD` rule:

```bash
mapfile -t sudoer_files < <(
  sudo grep -RIl \
    '^ostudy-admin[[:space:]].*NOPASSWD:ALL' \
    /etc/sudoers.d 2>/dev/null
)

printf '%s\n' "${sudoer_files[@]}"

for sudoer_file in "${sudoer_files[@]}"; do
  sudo sed -i \
    '/^ostudy-admin[[:space:]].*NOPASSWD:ALL/d' \
    "$sudoer_file"
done

sudo visudo -cf /etc/sudoers
sudo -k
sudo true
```

The final `sudo true` must require the new administrator password.

Verify UFW as administrator:

```bash
sudo ufw status verbose
```

Require active UFW and default deny incoming / allow outgoing.

## 10. Create the pre-registration snapshot

Stop nested emulators first:

```bash
sudo -u ostudy-runner -H bash -lc '
  set -a
  source /etc/ostudy-runner.env
  set +a
  cd /opt/ostudy-runner-bootstrap
  ./scripts/android-avd.sh stop
'
```

Power off:

```bash
sudo systemctl poweroff
```

Back on `ML-tower`, verify `shut off` before snapshotting:

```bash
virsh --connect qemu:///system domstate ostudy-ci-unified
```

Then:

```bash
virsh --connect qemu:///system snapshot-create-as \
  --domain ostudy-ci-unified \
  --name pre-github-runner \
  --description 'Pinned Linux/Android toolchains; no GitHub runner registration' \
  --disk-only \
  --atomic

virsh --connect qemu:///system start ostudy-ci-unified
```

If snapshot creation fails, stop and create a powered-off qcow2 backup instead of proceeding without rollback.

## 11. Download and register the Actions runner

Refresh the guest IP:

```bash
virsh --connect qemu:///system domifaddr ostudy-ci-unified
GUEST_IP=<current-guest-ip>
```

In GitHub, open **Repository Settings → Actions → Runners → New self-hosted runner**, choose Linux x64, and use GitHub's current download/extraction commands.

Open an interactive admin SSH session:

```bash
ssh -t -i ~/.ssh/ostudy-ci-unified \
  ostudy-admin@"$GUEST_IP"
```

Enter the runner shell:

```bash
sudo -u ostudy-runner -H bash -l
```

Inside that shell:

```bash
mkdir -p ~/actions-runner
cd ~/actions-runner
```

Run GitHub's current download and extraction commands here. Do not reuse another runner's binary directory or token.

Load the service environment before configuration:

```bash
set -a
source /etc/ostudy-runner.env
set +a
```

Obtain the current one-time registration token from GitHub. Do not paste it into chat. Register:

```bash
./config.sh \
  --url https://github.com/jferments/ostudy \
  --token <ONE-TIME-TOKEN-FROM-GITHUB> \
  --name ostudy-ci-unified \
  --labels ostudy-unified \
  --work _work \
  --unattended
```

Verify registration/environment before installing the service:

```bash
test -f .runner
test -f .credentials
grep -E '^(JAVA_HOME|ANDROID_HOME|ANDROID_SDK_ROOT)=' .env
cat .path
```

The runner home is intentionally private. Do not loosen `/home/ostudy-runner` permissions merely so `ostudy-admin` can browse it.

Exit the nested runner shell to return to the interactive `ostudy-admin` session.

## 12. Install the runner service correctly

GitHub's `svc.sh` resolves the runner root from the **current working directory**, not from the path used to invoke the script. `ostudy-admin` also cannot directly traverse the private runner home.

Therefore run service operations through a root shell that explicitly changes to the runner root:

```bash
sudo bash -lc '
  cd /home/ostudy-runner/actions-runner
  ./svc.sh install ostudy-runner
'
```

Start it:

```bash
sudo bash -lc '
  cd /home/ostudy-runner/actions-runner
  ./svc.sh start
'
```

Check status:

```bash
sudo bash -lc '
  cd /home/ostudy-runner/actions-runner
  ./svc.sh status
'
```

Do not replace these with `sudo /home/ostudy-runner/actions-runner/svc.sh ...`; that leaves `pwd` at the administrator directory and `svc.sh` reports that it must run from the runner root.

Protect the service from `needrestart` interruptions:

```bash
sudo mkdir -p /etc/needrestart/conf.d

echo '$nrconf{override_rc}{qr(^actions\.runner\..+\.service$)} = 0;' \
  | sudo tee /etc/needrestart/conf.d/actions_runner_services.conf
```

GitHub should show `ostudy-ci-unified` as Online/Idle with:

```text
self-hosted
Linux
X64
ostudy-unified
```

Do not add the historical `ostudy`, `android`, or `kvm` custom labels.

## 13. Shadow and production qualification

The infrastructure PR first uses `.github/workflows/unified-runner-qualification.yml` to prove:

```text
preflight
→ linux-fast
→ linux-full
→ android-full
→ all-full
```

Only after every shadow lane passes should production workflows move to `ostudy-unified`.

After the cutover workflow commit:

1. keep the PR draft while its automatic fast gate runs;
2. run shadow `all-full` on the cutover exact head;
3. mark the PR ready for review;
4. require the production `Check` workflow to run `all-full` successfully on the same exact head;
5. merge only after human review;
6. require the `main` push `Check` to pass `all-full` on `ostudy-ci-unified`.

## 14. Restore the pre-registration snapshot

A snapshot restore is destructive to guest state created after the snapshot. Do not restore while a runner job or nested emulator is active.

Stop the runner service and emulator, power off the VM, and then restore using the libvirt snapshot tools appropriate to the snapshot type.

The `pre-github-runner` snapshot predates GitHub runner registration. After restore, obtain a new one-time token and repeat registration/service installation.

## 15. Retire old runners

Do not remove old runners merely because the unified runner is online.

Retirement occurs only after the merged `main` commit completes authoritative `all-full` on `ostudy-ci-unified`.

Then:

- unregister the old runner entries in **Repository Settings → Actions → Runners** using GitHub's current removal instructions and one-time removal token if requested;
- discover the exact old service unit/install directory before deleting anything;
- stop/disable the identified old service;
- remove an old Multipass VM only after confirming it contains no unique data;
- keep `$HOME/Android/Sdk` on `ML-tower` if it is still useful for local/manual Android work.

Never use a broad wildcard deletion for runner directories or services.

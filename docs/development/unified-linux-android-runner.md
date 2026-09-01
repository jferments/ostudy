# Unified Linux and Android self-hosted runner

Status: current unified runner architecture. PR #49 establishes the production workflow cutover after exact-head shadow qualification.

## Purpose

ostudy uses one isolated Ubuntu 24.04 virtual machine on `ML-tower` for both Linux/Tauri and Android CI. The VM is the security and failure-containment boundary for repository-controlled Actions jobs.

```text
ML-tower
└── libvirt / QEMU / KVM
    └── ostudy-ci-unified
        ├── GitHub Actions service: ostudy-ci-unified
        ├── Linux / Tauri qualification
        ├── Android SDK / NDK / Gradle
        └── Android Emulator using nested /dev/kvm
```

The runner is repository-scoped and has one custom label:

```text
ostudy-unified
```

Default GitHub labels provide `self-hosted`, `linux`, and `x64`.

## Qualified resources

Canonical VM resources:

```text
Guest OS:   Ubuntu 24.04 LTS x86_64
RAM:        64 GiB requested
vCPUs:      16
Disk:       150 GiB qcow2
Swap:       16 GiB
CPU mode:   host-passthrough
Network:    libvirt default NAT
Firmware:   UEFI / OVMF
```

The guest preflight accepts the small difference between requested virtual capacity and what Linux reports after filesystem/virtualization overhead.

## Security boundary

The VM must not receive:

- host filesystem shares;
- the host SSH private key;
- forwarded SSH-agent access;
- Docker or libvirt host-control sockets;
- personal ostudy databases or imported documents;
- release/Android signing credentials;
- unrelated cloud or personal credentials.

The `ostudy-runner` account is unprivileged, locked for direct SSH login, and only receives guest group membership needed for KVM. Administration uses the separate `ostudy-admin` account with SSH key authentication and ordinary password-protected sudo.

Guest UFW is enabled with default-deny incoming traffic. The VM is attached only to libvirt's private NAT network and has no host port forwarding.

## Nested Android acceleration

Nested virtualization is a hard qualification boundary, not a label assumption.

On the host:

- hardware virtualization is enabled;
- the AMD/Intel KVM module has nested virtualization enabled;
- libvirt exposes the VM CPU through `host-passthrough`.

Inside the guest:

- `svm` or `vmx` is visible;
- `/dev/kvm` is usable by `ostudy-runner`;
- `kvm-ok` succeeds;
- Android Emulator acceleration succeeds;
- the pinned API 36 x86_64 AVD exists;
- the authoritative emulator smoke suite passes.

If nested acceleration or Android lifecycle behavior regresses after host/kernel/QEMU/Emulator changes, treat that as runner breakage. Do not weaken application or instrumentation assertions to make CI green.

## Toolchain model

Guest bootstrap installs repository-pinned:

- Node/npm/pnpm;
- Rust/Cargo and Android Rust targets;
- Linux/Tauri native dependencies and `tauri-driver`;
- JDK 17;
- Android command-line tools, platform, Build Tools, NDK, Emulator, and API 36 system image.

`/etc/ostudy-runner.env` is the non-interactive service environment. Runner registration captures the important tool paths in the Actions runner `.env` and `.path`.

Persistent reproducible caches live under the runner home rather than inside the clean Actions checkout. Checkout cleaning must never delete runner registration state, Android SDK/AVD state, or the pre-registration snapshot.

Native WebDriver smoke is an X11 functional contract, not a GPU-acceleration
benchmark. The guest intentionally has no DRI3 device, so `smoke-native.sh`
pins the X11 backend, disables WebKit accelerated/DMABUF compositing, and forces
Mesa software rendering before every isolated driver/application attempt. This
prevents WebKitWebDriver transport resets caused by probing unavailable graphics
hardware. The separate UI performance gate records its own fixed rendering
environment and retains the immutable Stage 0 comparison contract.

## CI architecture

One runner can execute only one Actions job at a time, so authoritative CI is deliberately serialized.

The primary `Check` workflow runs:

```text
guest preflight
→ Linux/shared gate
→ Android compile/package gate
→ Android emulator smoke
```

Draft pull requests use the fast gate for iteration. Ready pull requests and pushes to `main` run the complete `all-full` sequence.

The separate historical automatic `Android check` workflow is retired. Keeping separate automatic Linux and Android jobs on a single runner would only add queueing, repeated checkout/setup work, and ambiguous runner-label routing.

The opt-in `Unified runner qualification` workflow remains available for runner health/shadow qualification and infrastructure work.

## Snapshot and recovery

A powered-off libvirt snapshot named:

```text
pre-github-runner
```

was created after toolchain/bootstrap qualification and hardening but before GitHub runner registration.

Before any snapshot restore:

1. stop the Actions service;
2. stop nested Android emulator processes;
3. power off the VM.

Restoring the pre-registration snapshot intentionally removes the GitHub runner registration/service state created afterward. Obtain a new one-time repository runner token and register the service again after restoring.

The snapshot is not a source-code backup. Repository source is disposable and should be refreshed from GitHub or a verified Git bundle.

## Old runner retirement and rollback

The old Linux and bare-metal Android runners remain rollback assets until the unified production workflow proves itself on `main`.

Retirement requires:

1. local qualification inside the unified VM;
2. shadow `preflight`, `linux-fast`, `linux-full`, `android-full`, and `all-full` success;
3. production workflow cutover to `ostudy-unified`;
4. exact-head ready-for-review `all-full` success;
5. PR merge; and
6. successful `main` push `all-full` on `ostudy-ci-unified`.

Only then unregister and remove the old runner services. Keep the host Android SDK unless there is a separate reason to remove it.

If production qualification fails after cutover, stop changing the runner, inspect exact-head diagnostics, and either repair the unified VM or revert workflow routing while the old infrastructure is still available.

## Operating procedures

Provisioning, registration, service management, and recovery commands are maintained in [Unified runner setup](unified-runner-setup.md).

Test-tier and workflow behavior is maintained in [Testing and CI](testing-and-ci.md).

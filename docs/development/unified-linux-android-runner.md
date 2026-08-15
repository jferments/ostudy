# Unified Linux and Android self-hosted runner

Status: proposed and under staged implementation on a draft pull request.

This document defines the replacement for the current split Linux/Android runner arrangement. It is intentionally limited to CI, test orchestration, virtual-machine provisioning, runner security, and operational documentation. It does **not** change ostudy application behavior, Android product code, Android instrumentation semantics, database behavior, UI behavior, or existing test assertions.

## Goals

Create one isolated Ubuntu 24.04 LTS virtual machine on `ML-tower` that can execute both the authoritative Linux/Tauri qualification and the Android build/emulator qualification.

The VM will have:

- 64 GiB RAM;
- 150 GiB virtual disk;
- 16 GiB persistent swap;
- a configurable vCPU count, initially 16 unless the host preflight recommends a smaller value;
- nested KVM exposed through a libvirt `host-passthrough` CPU model;
- one repository-scoped GitHub Actions runner service;
- no host filesystem shares, USB passthrough, Docker socket, SSH-agent forwarding, or release-signing credentials;
- NAT networking with no inbound port forwarding;
- an unprivileged dedicated runner account;
- pinned Node, pnpm, Rust, JDK, Android SDK/NDK, emulator, and project dependencies derived from repository configuration;
- bounded persistent caches outside the checkout;
- disk, memory, KVM, toolchain, and runner-health preflights before expensive work.

## Non-goals

This work will not:

- add new Android application features;
- rewrite or broaden Android instrumentation tests;
- change system-picker behavior, Back behavior, lifecycle behavior, or the test-only Android bridge;
- alter database migrations or application state;
- introduce Codex or another code-writing agent inside the runner;
- install release signing keys;
- build Windows, macOS, or iOS infrastructure;
- immediately delete the existing runners before the replacement has passed shadow qualification;
- weaken the existing full Linux or Android merge gates.

## Why a single VM

A single runner service means GitHub Actions jobs execute serially. That matches the available physical host and avoids competing Rust, Gradle, Tauri, and emulator workloads exhausting memory or disk. Linux and Android share most of the source tree and toolchain, so one VM also avoids duplicating Node, pnpm, Rust, Cargo, JDK, Android SDK, NDK, Gradle, PDFium, and repository caches.

The VM is the containment boundary. Repository scripts may write broadly inside the guest, but they receive no host-mounted directory and no host credentials. A failed build can fill or damage the guest disk without directly modifying `ML-tower`'s home directory. The VM can be restored from a known-good snapshot if its state becomes suspect.

## Host and guest topology

```text
ML-tower (L0, Ubuntu Linux)
└── libvirt/QEMU/KVM
    └── ostudy-ci-unified (L1, Ubuntu 24.04 LTS)
        ├── GitHub runner: ostudy-ci-unified
        ├── Linux/Tauri build and native smoke
        ├── Android SDK/NDK/Gradle
        └── Android Emulator using nested /dev/kvm (L2)
```

The VM uses libvirt's default NAT network. It is not bridged onto the LAN and receives no host directory through 9P, virtiofs, NFS, Samba, or another sharing mechanism.

## Security model

### Trust boundary

The self-hosted runner executes repository-controlled shell commands. Therefore the runner VM is treated as disposable infrastructure, not as an extension of the host user account.

The guest must not contain:

- personal files;
- the host SSH private key;
- a forwarded SSH agent;
- GitHub personal access tokens stored on disk;
- release or Android signing credentials;
- cloud-provider credentials;
- access to the host Docker/libvirt sockets;
- host filesystem mounts;
- the live ostudy library/database.

### GitHub permissions

Pull-request workflows use the minimum token permissions needed to read the repository and upload workflow artifacts. They do not use `pull_request_target`, do not receive release secrets, do not push commits, and do not merge pull requests.

The runner is repository-scoped and receives one unique custom label:

```text
ostudy-unified
```

The VM may also temporarily receive `ostudy`, `android`, and `kvm` while existing workflows are shadow-tested, but the final workflow configuration will route all Linux and Android jobs through `ostudy-unified` so runner intent is unambiguous.

### Guest hardening

The guest uses:

- SSH keys only;
- disabled root SSH login;
- disabled password authentication;
- UFW default-deny inbound policy;
- no forwarded ports from the libvirt NAT network;
- unattended security updates outside active CI windows;
- a dedicated `ostudy-runner` account;
- membership only in groups required for its work, including `kvm` inside the guest;
- no general passwordless sudo after initial provisioning;
- a persistent systemd runner service;
- a clean checkout (`actions/checkout` clean/reset plus repository cleanliness checks) for every job;
- a known-good libvirt snapshot taken before GitHub registration secrets are introduced.

The runner is persistent during the first implementation because that is simpler to operate and preserves expensive build caches. The architecture leaves room for snapshot reset or ephemeral registration later, but that is not required to bring the replacement online.

## Resource allocation

The requested allocation is canonical:

```text
RAM:       64 GiB
Disk:      150 GiB qcow2 virtual disk
Swap:      16 GiB persistent swapfile
vCPUs:     16 by default, configurable during creation
Firmware:  UEFI/OVMF when available
CPU mode:  host-passthrough
Network:   libvirt default NAT, virtio NIC
Disk bus:  virtio
```

A 150 GiB virtual disk is thin-provisioned by qcow2 but may grow toward its full virtual size. The host preflight must therefore verify enough physical free space before VM creation.

## Nested KVM requirements

On `ML-tower` (L0):

1. hardware virtualization must be enabled;
2. the appropriate `kvm_intel` or `kvm_amd` module must be loaded;
3. the module's `nested` parameter must report `Y` or `1`;
4. libvirt must define the guest with `host-passthrough` or an equivalent CPU mode that exposes `vmx` or `svm` to the guest.

Inside the VM (L1):

1. `/proc/cpuinfo` must contain `vmx` or `svm`;
2. `/dev/kvm` must exist;
3. the `ostudy-runner` user must have read/write access to `/dev/kvm`;
4. `kvm-ok` must report that acceleration is available;
5. the Android Emulator acceleration check must succeed before the runner is registered.

The parent VM must not be saved, migrated, or reverted while an Android emulator is running. Stop the runner and nested emulator before snapshot operations.

## Staged implementation roadmap

### P0 — Recover the known-green baseline

- Start from current `main`, not from PR #48 or its deleted branch.
- Preserve all application source and existing Android instrumentation unchanged.
- Record current Linux and Android authoritative commands and workflow behavior.
- Add an explicit path guard that prevents this infrastructure PR from modifying application directories.
- Keep existing workflows operational until the replacement runner is verified.

Acceptance:

- The PR diff contains only CI workflows, runner/provisioning scripts, repository-policy tests for those scripts, and documentation.
- No files under `src/`, `src-tauri/src/`, Android app `src/main/`, or Android `src/androidTest/` are changed.

### P1 — Host preflight and VM creation

Add maintained scripts under `scripts/runner/` that:

- detect Intel versus AMD KVM;
- verify or explain how to enable nested virtualization;
- verify libvirt's default NAT network;
- verify at least 64 GiB allocatable RAM plus host headroom;
- verify at least 170 GiB free space before creating the 150 GiB guest disk;
- download and checksum an Ubuntu 24.04 LTS cloud image;
- generate cloud-init data without embedding secrets in Git;
- create `ostudy-ci-unified` with 64 GiB RAM, 150 GiB disk, 16 vCPUs, virtio devices, NAT, and `host-passthrough` CPU;
- enable libvirt autostart;
- avoid all host shares and passthrough devices.

Acceptance:

- `virsh dominfo ostudy-ci-unified` reports the requested resources.
- `virsh dumpxml` shows `host-passthrough`, the default NAT network, and no filesystem/hostdev entries.
- The guest boots and is reachable from the host using its generated SSH key.

### P2 — Guest bootstrap

A guest bootstrap script will:

- create/verify the 16 GiB swapfile;
- install Linux/Tauri native build prerequisites;
- install JDK 17;
- install the exact Node, npm, pnpm, Rust, Cargo, and Tauri versions required by `versions.env` and repository manifests;
- install Android command-line tools using the repository-pinned archive/checksum;
- install the pinned Android platform, Build Tools, NDK, platform tools, emulator, and API 36 x86_64 system image;
- install all four Android Rust targets;
- create the pinned Android AVD;
- provision persistent but bounded Cargo, Gradle, pnpm, Android, and PDFium caches;
- configure log rotation and a disk-cleanup timer;
- run Linux, nested-KVM, and Android runner doctors.

Acceptance:

- non-interactive service shells see all required tools;
- `/dev/kvm` is usable by `ostudy-runner`;
- the official Android emulator boots inside the VM;
- repository doctor scripts pass before GitHub registration.

### P3 — Register the runner without cutting over CI

Register one repository runner as `ostudy-ci-unified` with custom label `ostudy-unified`. Install it as a systemd service.

A new **manual-only** runner qualification workflow will target the new label and verify:

- CPU, RAM, disk, swap, and network;
- nested KVM and Android emulator acceleration;
- pinned toolchains;
- clean checkout semantics;
- Linux static/fast qualification;
- Android environment/project doctors;
- one explicitly requested authoritative Linux or Android command.

This workflow will not trigger on ordinary pull requests until the VM is proven.

Acceptance:

- GitHub shows the runner online and idle;
- manual runner preflight passes;
- manual Linux qualification passes;
- manual Android qualification passes on the unchanged application/test baseline.

### P4 — CI orchestration design

Replace duplicated runner jobs with one serialized qualification job on the unified runner. A single checkout avoids repeated setup and allows Android build and emulator steps to share the same exact artifacts without uploading and downloading them between jobs.

The orchestrator will use the existing, known-green repository commands as opaque authoritative boundaries:

```text
check-static.sh
check-fast.sh
check.sh
android-test.sh
android-emulator-smoke.sh
```

The first workflow migration will not rewrite those scripts or change their test semantics.

A conservative classifier will select work by changed path, but the workflow itself will always start so required checks never remain pending because a `paths:` filter skipped the workflow.

Suggested development order:

```text
1. runner/resource preflight
2. repository and static contracts
3. TypeScript/Rust fast qualification
4. full Linux/Tauri qualification when required
5. Android compile/package qualification when required
6. Android emulator smoke when required
7. final exact-head summary
```

Ready-for-review and `main` qualification remain authoritative. Draft iteration may use narrower routes, but a narrow route must never be reported as full qualification.

### P5 — Efficiency without missing failures

The efficient design follows these rules:

1. **Fail cheaply first.** Repository policy, generated files, lockfiles, formatting, and type checks run before Rust/Tauri/Gradle/emulator work.
2. **Do not duplicate portable suites.** Shared TypeScript and Rust tests run once per qualification, not once in a Linux job and again in an Android job unless an Android-specific command genuinely depends on them.
3. **One checkout and one job.** With one runner, parallel jobs cannot execute concurrently; separate jobs only add queue, checkout, installation, and artifact-transfer overhead.
4. **Reuse exact artifacts within the job.** The x86_64 APK and instrumentation package built for emulator testing are the artifacts installed into the emulator.
5. **Cancel obsolete heads.** Workflow concurrency cancels older runs for the same PR/ref.
6. **Cache outside the checkout.** Cargo target, pnpm store, Gradle caches, Android SDK/NDK/system images, and provisioned native runtimes persist across clean checkouts.
7. **Bound caches.** A timer removes old workspaces and cache generations before the disk reaches a critical threshold.
8. **Keep diagnostics observational.** Result summaries and artifacts describe existing commands; they do not introduce new application tests or change test behavior.
9. **Targeted reruns are development-only.** A failed narrow test may be rerun for diagnosis, but the final exact head must pass complete required gates.
10. **No hidden success.** `queued`, `running`, `skipped`, and `blocked` are not passes.

### P6 — Shadow cutover

- Run the new unified workflow manually against the same commit as the current workflows.
- Compare command output, artifacts, runtime, and results.
- Temporarily pause old runners rather than deleting them.
- Route a draft PR through the unified runner.
- Route a ready PR through the unified runner.
- Confirm Linux native smoke and Android emulator smoke both pass from a clean checkout.

Only after parity is demonstrated will existing workflow labels be changed.

### P7 — Final cutover and old-runner retirement

- Change Linux, Android, packaging, and relevant release workflow runner selectors to `ostudy-unified` where appropriate.
- Preserve release/signing separation; the unified PR runner remains unsigned and unprivileged.
- Verify exact-head CI on the cutover commit.
- Stop and unregister old GitHub runner services.
- Remove the old bare-metal runner directory and service.
- Remove the old Multipass VM only after its data and logs are confirmed disposable.
- Document rollback: re-enable the old runner or revert runner labels while the unified VM remains under investigation.

## Workflow architecture

### One serialized job

Because one runner accepts one job at a time, the primary PR workflow should use a single job rather than separate Linux compile, Android compile, and emulator jobs. This prevents the same VM from queueing work behind itself and avoids rebuilding or transferring artifacts.

### Always-run classifier

The workflow triggers for every PR to `main`. Its first script computes a conservative lane:

```text
docs-static
shared-fast
linux-full
android-full
linux-and-android-full
```

Examples:

- documentation-only changes: static/documentation contracts;
- frontend-only UI changes: shared fast plus Linux full when ready;
- Rust/domain changes: Linux full and Android full when ready because both platforms share Rust state;
- Android project/plugin/script changes: shared fast plus Android full;
- workflow/runner/script changes: Linux and Android full;
- unknown or mixed changes: Linux and Android full.

The classifier is an optimization, not a correctness authority. It escalates when uncertain.

### Draft versus authoritative behavior

Draft PRs may execute `check-changed.sh` or a conservative fast lane. Ready-for-review PRs execute the complete selected authoritative lane. `main` pushes execute the complete selected authoritative lane again.

Manual dispatch provides explicit modes:

```text
preflight
static
fast
linux-full
android-full
all-full
```

### Evidence

Every run publishes a concise summary containing:

- branch and exact head SHA;
- runner name and labels;
- selected lane and reason;
- commands executed;
- start/end/duration;
- pass/fail/skip status;
- bounded failure highlights;
- links/names for retained logs and native diagnostics.

Android failures retain existing APK, Gradle, Logcat, screenshot, and hierarchy evidence. Linux native failures retain the existing diagnostic bundle. No user study data or provider credentials are included.

## Disk and cache operations

The VM starts with a 150 GiB disk and must keep at least 25 GiB free. Before expensive qualification:

```text
>= 40 GiB free: normal
25–40 GiB free: warn and prune stale workspaces/cache generations
< 25 GiB free: fail preflight before building
```

The cleanup policy targets only reproducible state:

- completed Actions workspaces not associated with the active job;
- old Cargo target fingerprints;
- old Gradle transform/build caches;
- pnpm store generations no longer referenced;
- old Android emulator snapshots and crash logs;
- expired workflow diagnostic staging;
- package-manager caches.

It never deletes source branches, runner registration files, Android signing keys, or user data.

## Progress reporting and agent loop

GitHub Actions should publish enough structured evidence that a Connector-backed coding agent can inspect the current head without asking the user to copy terminal output. During implementation the agent reports each iteration as:

```text
Head:
Workflow/run inspected:
Failure batch:
Repair applied:
Next qualification:
```

A future event-driven status/comment mechanism may be added after the unified runner is stable. It is not part of the initial runner bring-up and will not be allowed to execute PR code with elevated permissions.

## Initial local setup sequence

The repository will provide scripts for these steps. Until those scripts land, the intended sequence is:

1. Verify host virtualization and nested KVM.
2. Install `qemu-kvm`, `libvirt-daemon-system`, `libvirt-clients`, `virtinst`, `cloud-image-utils`, `cpu-checker`, and `qemu-utils` on `ML-tower`.
3. Add the host user to `libvirt` and `kvm`, then log out/in.
4. Create a dedicated SSH key used only to administer this VM.
5. Run the host preflight.
6. Run the VM creation script with 64 GiB RAM, 150 GiB disk, 16 GiB swap, and the chosen vCPU count.
7. Verify nested KVM inside the guest.
8. Clone this branch inside the guest and run the guest bootstrap script.
9. Run repository doctors and one clean Linux and Android qualification locally inside the guest.
10. Register the GitHub runner only after all local pre-registration checks pass.

## Rollback

Before runner registration, create a clean libvirt snapshot named `pre-github-runner`. Before workflow cutover, create a second snapshot named `pre-ci-cutover` while the guest and nested emulator are stopped.

If a CI change fails:

1. revert the workflow change or restore the previous runner label;
2. stop the runner service;
3. stop any nested emulator;
4. restore the known-good VM snapshot if guest state is suspect;
5. keep old runners available until the replacement passes both Linux and Android authoritative gates.

## Completion criteria

The infrastructure is complete only when:

- the unified VM matches the requested resource allocation;
- nested KVM and the Android emulator work inside the VM;
- Linux static, fast, full, Tauri build, and native smoke pass;
- Android environment, compile/package, Gradle tests, APK checks, and emulator smoke pass without changing application/test behavior;
- the unified workflow qualifies a ready PR and a `main` push;
- disk/cache protections are verified;
- the VM can be rebuilt from repository documentation and scripts;
- old runners are removed only after an explicit human-approved cutover.

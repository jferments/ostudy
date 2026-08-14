# Android self-hosted runner setup

ostudy's Android CI uses a Linux x86_64 self-hosted GitHub Actions runner. The Android compile/package job requires these labels:

```text
self-hosted
linux
x64
ostudy
android
```

The official-emulator job is deliberately stricter and additionally requires:

```text
kvm
```

The `android` and `kvm` labels are assertions made by the maintainer: GitHub does not test the host's Android SDK or virtualization capabilities when a label is assigned. `scripts/android-runner-doctor.sh` performs those checks at job runtime.

## Use a host-level runner for the emulator

Run the Android/KVM runner directly on the Linux x86_64 host where `/dev/kvm` is available to the runner account. Android's emulator documentation does not support VM-accelerated emulator execution nested inside another VM. A separate virtual-machine runner can continue to handle ordinary Linux CI, but it should not receive the `kvm` label merely because KVM is installed on the outer host.

The Android host needs:

- the normal ostudy Linux build prerequisites;
- JDK 17;
- Node.js, pnpm, Rust, and rustup at the versions pinned by the repository;
- Android command-line tools and Emulator;
- direct KVM access for the runner service account.

Android Studio may manage the SDK, but the IDE is not required to be running. On Linux, `scripts/android-env.sh` automatically reuses `$HOME/Android/Sdk` when `ANDROID_HOME` and `ANDROID_SDK_ROOT` are unset. The repository installer adds its pinned API 36 platform, Build Tools 36.0.0, NDK 28.2, and API 36 Google APIs x86_64 image alongside any newer SDK packages already installed.

The current Android project deliberately remains on API 36 even if API 37 is installed on the host. API 37 requires a newer Android Gradle Plugin line than the A1–A3 project currently pins, so API 37 can remain installed side-by-side without changing the qualified build matrix.

## Prepare the runner account

Run these commands as the same account that will own the GitHub Actions runner service:

```bash
sudo usermod -aG kvm "$USER"
```

Log out and back in after changing group membership, then verify:

```bash
id
ls -l /dev/kvm
```

If Android Studio placed the SDK somewhere other than `$HOME/Android/Sdk`, export its location before using the repository tools:

```bash
export ANDROID_HOME=/absolute/path/to/Android/Sdk
export ANDROID_SDK_ROOT="$ANDROID_HOME"
```

From a checkout of ostudy:

```bash
pnpm install --frozen-lockfile
pnpm android:install-deps
rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  i686-linux-android \
  x86_64-linux-android
pnpm android:runner:doctor
pnpm android:avd create
```

Perform one clean local emulator qualification before registering the runner:

```bash
pnpm android:smoke
```

This proves that SDK selection, KVM, AVD creation, Rust cross-compilation, Gradle, APK installation, instrumentation, and Android system UI all work under the runner account.

## Register the GitHub runner

In the repository on GitHub, open **Settings → Actions → Runners → New self-hosted runner** and choose Linux x64. GitHub displays the current runner download and one-time registration-token commands. Use those commands rather than copying an old runner binary or registration token.

When running `config.sh`, add the ostudy Android labels. The relevant part is:

```bash
./config.sh \
  --url https://github.com/jferments/ostudy \
  --token <ONE_TIME_TOKEN_FROM_GITHUB> \
  --name ostudy-android \
  --labels ostudy,android,kvm \
  --work _work
```

GitHub automatically supplies the `self-hosted`, `linux`, and `x64` default labels for a normal Linux x64 runner. Do not use `--no-default-labels` for this runner.

If an older host-level runner is already registered without the required custom labels, add `android` and `kvm` from the GitHub runner settings. If the existing runner is inside a VM, keep it for ordinary Linux work and register a separate host-level Android runner instead. The workflow will not send emulator jobs to a runner until every label in `runs-on` matches.

## Run the runner as a service

After registration, install the runner as a system service under the same account that owns the Android SDK, AVD, Rust toolchain, and KVM membership:

```bash
sudo ./svc.sh install "$USER"
sudo ./svc.sh start
sudo ./svc.sh status
```

On Debian/Ubuntu hosts with `needrestart`, keep package upgrades from restarting the runner in the middle of a job:

```bash
echo '$nrconf{override_rc}{qr(^actions\.runner\..+\.service$)} = 0;' \
  | sudo tee /etc/needrestart/conf.d/actions_runner_services.conf
```

Because a systemd service does not necessarily read interactive shell startup files, prefer the repository's SDK auto-detection or set persistent service environment variables if the SDK is in a nonstandard location. The runner must see JDK 17, `rustup`, `pnpm`, and the Android SDK when invoked non-interactively.

## Verify service execution

With the service running:

```bash
sudo ./svc.sh status
```

GitHub's runner page should show the runner as **Idle** and list `self-hosted`, `linux`, `x64`, `ostudy`, `android`, and `kvm`.

Then make or synchronize a pull request against `main`. `.github/workflows/android-check.yml` automatically performs:

1. Android project verification;
2. shared TypeScript and Rust tests;
3. x86_64 APK and instrumentation-package compilation;
4. package, permission, and 16 KiB native-library checks;
5. a clean official API 36 emulator boot;
6. native smoke tests for library creation, practice, analytics, persistence, Android Back, process restoration, and system document pickers;
7. diagnostic APK/Gradle artifacts and emulator Logcat/screenshots on failure.

The emulator job wipes its AVD data by default, so CI does not depend on state left by a previous test run.

## Troubleshooting a queued job

A job that remains queued usually has no online runner matching every requested label. Check the repository's runner page first. For the emulator job, verify the exact label set and run:

```bash
pnpm android:runner:doctor
```

If the runner is online but the emulator doctor reports no KVM access, fix `/dev/kvm` ownership/group membership and restart the runner service after the service account receives the `kvm` group.

If an emulator job is still being routed to a virtual-machine runner, remove its `kvm` label and register the host-level Android runner instead.

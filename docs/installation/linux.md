# Install and run ostudy on Linux

Ubuntu and Debian are the primary supported development environment. The repository setup script installs the packages and pinned toolchains expected by local checks and the self-hosted Linux runner, then creates a standalone **oStudy Dev** build using `io.github.jferments.ostudy.dev`. This source-checkout path is intentionally isolated from the production `io.github.jferments.ostudy` library; it is not the production package installer.

## Prerequisites

Use an Ubuntu or Debian system with:

- an account that can run `sudo apt-get`;
- network access for operating-system packages, pinned language toolchains, JavaScript packages, Rust crates, and the verified PDFium archive;
- enough disk space for Node.js, Rust, Cargo build output, and application dependencies;
- a graphical session capable of running a WebKitGTK application.

The setup script installs the WebKitGTK, AppIndicator, SVG, OpenSSL, X11/WebDriver, Openbox, `x11-utils`, SQLite, Python, and ordinary build packages used by ostudy. It installs the exact Node.js, pnpm, Rust, rustfmt, Clippy, and `tauri-driver` versions recorded in `versions.env`.

## One-time setup

From the repository root:

```bash
./scripts/setup-ubuntu.sh
```

The script is intentionally repeatable. Existing matching tool installations are reused, while version mismatches fail or are replaced explicitly rather than being ignored.

The ordinary build path provisions the pinned PDFium runtime needed to render PDF pages. To enable OCR fallback for PDFs whose embedded text is unusable, also install Tesseract and its English data through the reviewed native dependency installer:

```bash
./scripts/install-native-deps.sh --with-tesseract
```

On Ubuntu/Debian this uses the distribution's `tesseract-ocr` and `tesseract-ocr-eng` packages. Normal text-based PDFs do not require OCR, but corrupted or image-only text layers may.

## Start the application

After setup:

```bash
./run-app.sh
```

`run-app.sh` launches the existing standalone debug binary. It deliberately does not start Vite or rebuild the application. This keeps normal startup fast and makes it obvious when a build is stale.

After changing source code:

```bash
./scripts/build.sh
./run-app.sh
```

For a live development session instead:

```bash
pnpm install --frozen-lockfile
pnpm desktop
```

`pnpm desktop` runs Tauri hot reload through `scripts/dev.sh` with the development overlay, so it also uses `io.github.jferments.ostudy.dev` and its separate library. Install first: pnpm refuses to run scripts while `node_modules` is older than the lockfile. Do not run bare `pnpm tauri dev`; that debug build resolves the production identifier and startup refuses it. The live development command is useful while editing the interface and is how the maintainer tries a branch; the standalone build is the artifact for testing the built application.

## Control compilation parallelism

Cargo normally uses all detected logical processors. On a machine where that competes with other work or memory, set an explicit limit:

```bash
CARGO_BUILD_JOBS=8 ./scripts/build.sh
```

The document extraction and OCR worker counts are separate runtime decisions. They are chosen from task size, CPU count, and available memory, and can be capped with the documented operator environment variables when diagnosing a machine-specific problem.

## Validate the checkout

Use the least expensive check that answers the current question:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

The full gate builds the native application and exercises the real WebDriver persistence smoke test. See [Testing and CI](../development/testing-and-ci.md) before interpreting a narrower check as complete qualification.

## Common problems

### The launcher says the build is stale

Run:

```bash
./scripts/build.sh
```

The launcher compares the current source tree with the recorded build state and refuses to present an old binary as current.

### PDFium cannot be loaded

Re-run:

```bash
./scripts/install-native-deps.sh --pdfium-only
```

The installer verifies both the pinned archive and the staged native library. An operator-managed compatible library may be selected with `OSTUDY_PDFIUM_PATH`.

### OCR is unavailable

Confirm that Tesseract and English data are visible:

```bash
tesseract --version
tesseract --list-langs
```

The language list must contain `eng`. Re-run the installer with `--with-tesseract` when necessary.

### Environment diagnostics fail

Run:

```bash
./scripts/doctor.sh
```

The diagnostic output names the missing or mismatched tool instead of allowing a later build step to fail ambiguously.

### Open and save dialogs look like GTK, not your desktop

oStudy asks for your desktop's own file picker through the file chooser portal (`xdg-desktop-portal`, with the backend for your desktop such as `xdg-desktop-portal-kde` or `xdg-desktop-portal-gnome`). When no portal is running, it falls back to GTK's picker. Install your desktop's portal backend to get its picker. To keep GTK's picker, start oStudy with `GTK_USE_PORTAL=0`.


## Install or update the production packages

The production packages use the stable `io.github.jferments.ostudy` identity and its own library, separate from the `.dev` library above. Back up the installed app's library first (Settings → Backup/Sync, saved outside the app folder): a newer build upgrades the library once when it first opens it, and an older build cannot open it afterwards.

Build the Debian and RPM packages from the commit you have checked out:

```bash
./scripts/ostudy-dev package-deb
```

The packages, `BUILD-INFO.txt` (with the commit) and checksums land in `.artifacts/linux-package/`. Install or update with your distribution's package manager:

```bash
# Debian, Ubuntu and derivatives
sudo apt install --reinstall ./.artifacts/linux-package/ostudy-v0.1.0-linux-x86_64.deb
# Fedora and RHEL-family
sudo dnf install ./.artifacts/linux-package/ostudy-v0.1.0-linux-x86_64.rpm
# openSUSE
sudo zypper install --allow-unsigned-rpm ./.artifacts/linux-package/ostudy-v0.1.0-linux-x86_64.rpm
```

Every build is currently numbered `0.1.0`, so a newer build looks like the same version to the package manager. `apt install --reinstall` replaces it anyway; on Fedora use `sudo dnf reinstall ./…rpm` once the package is installed.

### Packages for every distribution

Packages built on a new distribution only run on systems at least as new: built on Ubuntu 24.04 they need glibc 2.39 and refuse to start on Debian 12 or Ubuntu 22.04. To build the release set that runs on every maintained major distribution, use Docker or Podman:

```bash
./scripts/package-linux-portable.sh
```

It builds the `.deb`, `.rpm`, AppImage and a `.tar.gz` folder build from the committed source inside an Ubuntu 22.04 container (glibc 2.35), runs the same package checks, and writes them with `BUILD-INFO.txt` and `SHA256SUMS.txt` to `.artifacts/linux-portable/`. Toolchains and build output are cached in the `ostudy-linux-portable-cache` container volume, so later runs are faster.

The AppImage carries WebKitGTK and GStreamer but, like every AppImage, expects the desktop's own graphics and font libraries (Mesa's GL, EGL and GLES, fontconfig) plus `libusb` and the system certificate store. The `.tar.gz` runs from any folder (`bin/ostudy`) and lists the system packages it needs in its `README.txt`.

### Flatpak

`./scripts/build-flatpak.sh` turns the verified `.deb` into a Flatpak on the GNOME runtime and writes a single-file bundle to `.artifacts/flatpak/`; `--install` also installs it for your user. It needs `flatpak` and `flatpak-builder` and downloads the runtime from Flathub the first time. With `--container` it builds inside a privileged Ubuntu 24.04 container (Docker or Podman) instead, caching the runtime in the `ostudy-flatpak-cache` volume.

```bash
./scripts/ostudy-dev package-deb
./scripts/build-flatpak.sh --install
flatpak run io.github.jferments.ostudy
```

The Flatpak keeps its library in `~/.var/app/io.github.jferments.ostudy/`, separate from the `.deb` or `.rpm` library; move a library between them with a whole-library backup and restore. Direct USB sync still needs the udev rule from the host (`src-tauri/packaging/linux/70-ostudy-android-usb.rules` in `/etc/udev/rules.d/`), and OCR fallback is not available inside the Flatpak yet.

## Production package and uninstall safety

The source-checkout workflow above is development-only. The production Debian and RPM packages are built with the stable `io.github.jferments.ostudy` identity and is intentionally separate from the `io.github.jferments.ostudy.dev` development profile.

The production package owns application binaries, desktop/MIME integration, bundled runtime resources, and the reviewed udev rule. It **does not own the learner library under the user's XDG data directory**. A normal package-manager removal must therefore remove package-owned program/integration files while leaving the user's oStudy library in place.

For the future public installer, these are separate operations:

```text
normal uninstall
  -> remove installed package/integration files
  -> preserve ~/.local/share/io.github.jferments.ostudy (or $XDG_DATA_HOME equivalent)
  -> preserve learner data

explicit purge-user-data
  -> separate user-level action
  -> never implied by apt/dnf/pacman remove
  -> validate the exact production application-data directory
  -> require explicit destructive confirmation and backup guidance
```

The repository does not yet expose a public `--purge-user-data` command. Do not substitute `rm -rf`, package maintainer scripts, or a root-owned home-directory cleanup for that future reviewed boundary.

Release generation statically verifies the package ownership policy and inspects the built `.deb` and `.rpm` before publication. Actual install/remove/reinstall preservation remains a separate acceptance test using disposable production data before any real school library is trusted to the installed profile.

`./scripts/ostudy-dev package-deb` (above) writes the verified `.deb` and `.rpm`, `BUILD-INFO.txt`, and `SHA256SUMS.txt` under `.artifacts/linux-package/`. It does not install or remove anything. Terminal CI runs the same package builder after full Linux qualification and retains that exact-SHA package on the self-hosted runner for bounded acceptance evidence.

## Other Linux distributions

The source is not intentionally tied to `apt`, but the complete bootstrap script currently is. On another distribution, install the equivalent Tauri/WebKitGTK build packages, the pinned language toolchains, PDFium, and Tesseract manually, then run the normal frozen/locked build commands. Such systems are not yet continuously qualified, so record distribution-specific fixes rather than adding silent fallbacks to the Ubuntu script.

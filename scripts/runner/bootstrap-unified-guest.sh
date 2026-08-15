#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/../.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"

ADMIN_USER="${OSTUDY_VM_ADMIN_USER:-ostudy-admin}"
RUNNER_USER="${OSTUDY_VM_RUNNER_USER:-ostudy-runner}"
RUNNER_HOME="/home/$RUNNER_USER"
BOOTSTRAP_SOURCE="/opt/ostudy-runner-bootstrap"
JAVA_HOME_VALUE="/usr/lib/jvm/java-${ANDROID_JDK_MAJOR}-openjdk-amd64"
ANDROID_HOME_VALUE="$RUNNER_HOME/Android/Sdk"
NODE_PREFIX="/opt/ostudy-toolchains/node-v$NODE_VERSION"
NODE_ARCHIVE="node-v$NODE_VERSION-linux-x64.tar.xz"
NODE_BASE_URL="https://nodejs.org/dist/v$NODE_VERSION"

if [[ "$(id -u)" == 0 ]]; then
  printf 'Run this script as the SSH administration user, not directly as root.\n' >&2
  exit 1
fi
if ! sudo -n true >/dev/null 2>&1; then
  printf 'Passwordless sudo is required during initial guest bootstrap.\n' >&2
  printf 'The cloud-init admin account provides this temporarily; runner registration remains separate.\n' >&2
  exit 1
fi
if [[ "$(uname -s):$(uname -m)" != Linux:x86_64 ]]; then
  printf 'Unified runner guest must be Linux x86_64.\n' >&2
  exit 1
fi
if [[ ! -d "$ROOT_DIR/.git" ]]; then
  printf 'Bootstrap source must be a real Git checkout with .git metadata.\n' >&2
  printf 'Transfer the infrastructure branch with the documented Git bundle procedure.\n' >&2
  exit 1
fi

printf 'Bootstrapping ostudy unified Linux/Android runner guest...\n'
printf '  admin user:  %s\n' "$ADMIN_USER"
printf '  runner user: %s\n' "$RUNNER_USER"
printf '  node:        %s\n' "$NODE_VERSION"
printf '  pnpm:        %s\n' "$PNPM_VERSION"
printf '  rust:        %s\n' "$RUST_VERSION"
printf '  JDK:         %s\n' "$ANDROID_JDK_MAJOR"
printf '  Android SDK: %s\n\n' "$ANDROID_HOME_VALUE"

sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y \
  build-essential \
  ca-certificates \
  clang \
  cmake \
  cpu-checker \
  curl \
  dbus-x11 \
  file \
  git \
  jq \
  libasound2t64 \
  libayatana-appindicator3-dev \
  libdbus-1-3 \
  libfontconfig1 \
  libgl1-mesa-dri \
  libgtk-3-dev \
  libjavascriptcoregtk-4.1-dev \
  libnss3 \
  libpulse0 \
  librsvg2-dev \
  libsoup-3.0-dev \
  libssl-dev \
  libwebkit2gtk-4.1-dev \
  libx11-6 \
  libx11-xcb1 \
  libxcb1 \
  libxcomposite1 \
  libxcursor1 \
  libxdamage1 \
  libxi6 \
  libxkbcommon0 \
  libxrandr2 \
  libxtst6 \
  openjdk-${ANDROID_JDK_MAJOR}-jdk-headless \
  patchelf \
  pkg-config \
  python3 \
  python3-pip \
  python3-venv \
  qemu-kvm \
  rsync \
  sqlite3 \
  tar \
  tesseract-ocr \
  tesseract-ocr-eng \
  unzip \
  webkit2gtk-driver \
  wget \
  xdg-utils \
  xvfb \
  xz-utils \
  zip

id "$RUNNER_USER" >/dev/null 2>&1 || {
  sudo useradd --create-home --shell /bin/bash "$RUNNER_USER"
}
sudo usermod -aG kvm "$RUNNER_USER"
sudo install -d -o "$RUNNER_USER" -g "$RUNNER_USER" -m 0750 \
  "$RUNNER_HOME/actions-runner" \
  "$RUNNER_HOME/.cache/ostudy/cargo-target" \
  "$RUNNER_HOME/.cache/gradle" \
  "$RUNNER_HOME/.cache/pnpm/store" \
  "$RUNNER_HOME/.cargo" \
  "$RUNNER_HOME/.rustup" \
  "$RUNNER_HOME/.local/bin" \
  "$ANDROID_HOME_VALUE"

work_dir="$(mktemp -d -t ostudy-guest-bootstrap.XXXXXXXX)"
cleanup() { rm -rf -- "$work_dir"; }
trap cleanup EXIT

printf 'Installing verified Node.js %s system-wide...\n' "$NODE_VERSION"
curl --fail --location --proto '=https' --tlsv1.2 \
  "$NODE_BASE_URL/SHASUMS256.txt" \
  --output "$work_dir/SHASUMS256.txt"
curl --fail --location --proto '=https' --tlsv1.2 \
  "$NODE_BASE_URL/$NODE_ARCHIVE" \
  --output "$work_dir/$NODE_ARCHIVE"
(
  cd "$work_dir"
  grep -F "  $NODE_ARCHIVE" SHASUMS256.txt | sha256sum --check --status
)
sudo rm -rf -- "$NODE_PREFIX"
sudo install -d -o root -g root -m 0755 "$NODE_PREFIX"
sudo tar -xJf "$work_dir/$NODE_ARCHIVE" -C "$NODE_PREFIX" --strip-components=1

# The npm launcher in the official Node archive uses `#!/usr/bin/env node`.
# Make the verified Node binary discoverable before invoking npm so a fresh or
# resumed bootstrap does not depend on a pre-existing system Node installation.
sudo ln -sfn "$NODE_PREFIX/bin/node" /usr/local/bin/node
sudo env PATH="$NODE_PREFIX/bin:/usr/local/bin:/usr/bin:/bin" \
  "$NODE_PREFIX/bin/npm" install --global "npm@$NPM_VERSION" "pnpm@$PNPM_VERSION"
for command_name in node npm npx corepack pnpm pnpx; do
  if [[ -e "$NODE_PREFIX/bin/$command_name" ]]; then
    sudo ln -sfn "$NODE_PREFIX/bin/$command_name" "/usr/local/bin/$command_name"
  fi
done

[[ "$(/usr/local/bin/node --version)" == "v$NODE_VERSION" ]] || {
  printf 'Installed Node version does not match versions.env.\n' >&2
  exit 1
}
[[ "$(/usr/local/bin/npm --version)" == "$NPM_VERSION" ]] || {
  printf 'Installed npm version does not match versions.env.\n' >&2
  exit 1
}
[[ "$(/usr/local/bin/pnpm --version)" == "$PNPM_VERSION" ]] || {
  printf 'Installed pnpm version does not match versions.env.\n' >&2
  exit 1
}

printf 'Installing Rust %s for %s...\n' "$RUST_VERSION" "$RUNNER_USER"
sudo -u "$RUNNER_USER" -H env \
  HOME="$RUNNER_HOME" \
  RUSTUP_HOME="$RUNNER_HOME/.rustup" \
  CARGO_HOME="$RUNNER_HOME/.cargo" \
  bash -Eeuo pipefail -c '
    curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --profile minimal --default-toolchain "$1"
  ' _ "$RUST_VERSION"

sudo -u "$RUNNER_USER" -H env \
  HOME="$RUNNER_HOME" \
  RUSTUP_HOME="$RUNNER_HOME/.rustup" \
  CARGO_HOME="$RUNNER_HOME/.cargo" \
  PATH="$RUNNER_HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin" \
  rustup component add --toolchain "$RUST_VERSION" rustfmt clippy
sudo -u "$RUNNER_USER" -H env \
  HOME="$RUNNER_HOME" \
  RUSTUP_HOME="$RUNNER_HOME/.rustup" \
  CARGO_HOME="$RUNNER_HOME/.cargo" \
  PATH="$RUNNER_HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin" \
  rustup target add --toolchain "$RUST_VERSION" \
    aarch64-linux-android \
    armv7-linux-androideabi \
    i686-linux-android \
    x86_64-linux-android

sudo tee /etc/ostudy-runner.env >/dev/null <<EOF
CI=true
HOME=$RUNNER_HOME
USER=$RUNNER_USER
LOGNAME=$RUNNER_USER
JAVA_HOME=$JAVA_HOME_VALUE
ANDROID_HOME=$ANDROID_HOME_VALUE
ANDROID_SDK_ROOT=$ANDROID_HOME_VALUE
RUSTUP_HOME=$RUNNER_HOME/.rustup
CARGO_HOME=$RUNNER_HOME/.cargo
GRADLE_USER_HOME=$RUNNER_HOME/.cache/gradle
PNPM_HOME=$RUNNER_HOME/.local/share/pnpm
OSTUDY_CARGO_TARGET_DIR=$RUNNER_HOME/.cache/ostudy/cargo-target
PATH=$JAVA_HOME_VALUE/bin:$ANDROID_HOME_VALUE/platform-tools:$ANDROID_HOME_VALUE/emulator:$ANDROID_HOME_VALUE/cmdline-tools/latest/bin:$RUNNER_HOME/.cargo/bin:$RUNNER_HOME/.local/bin:/usr/local/bin:/usr/bin:/bin
EOF
sudo chmod 0644 /etc/ostudy-runner.env

sudo tee /etc/profile.d/ostudy-runner.sh >/dev/null <<'EOF'
if [[ "${USER:-}" == "ostudy-runner" ]]; then
  set -a
  # shellcheck disable=SC1091
  source /etc/ostudy-runner.env
  set +a
fi
EOF
sudo chmod 0644 /etc/profile.d/ostudy-runner.sh

printf 'Copying the current Git checkout into a disposable qualification tree...\n'
sudo rm -rf -- "$BOOTSTRAP_SOURCE"
sudo install -d -o "$RUNNER_USER" -g "$RUNNER_USER" -m 0750 "$BOOTSTRAP_SOURCE"
sudo rsync -a --delete \
  --exclude node_modules \
  --exclude dist \
  --exclude src-tauri/target \
  --exclude src-tauri/gen/android/.gradle \
  --exclude src-tauri/gen/android/app/build \
  --chown="$RUNNER_USER:$RUNNER_USER" \
  "$ROOT_DIR/" "$BOOTSTRAP_SOURCE/"

runner_env=(
  HOME="$RUNNER_HOME"
  USER="$RUNNER_USER"
  LOGNAME="$RUNNER_USER"
  JAVA_HOME="$JAVA_HOME_VALUE"
  ANDROID_HOME="$ANDROID_HOME_VALUE"
  ANDROID_SDK_ROOT="$ANDROID_HOME_VALUE"
  RUSTUP_HOME="$RUNNER_HOME/.rustup"
  CARGO_HOME="$RUNNER_HOME/.cargo"
  GRADLE_USER_HOME="$RUNNER_HOME/.cache/gradle"
  PNPM_HOME="$RUNNER_HOME/.local/share/pnpm"
  OSTUDY_CARGO_TARGET_DIR="$RUNNER_HOME/.cache/ostudy/cargo-target"
  PATH="$JAVA_HOME_VALUE/bin:$ANDROID_HOME_VALUE/platform-tools:$ANDROID_HOME_VALUE/emulator:$ANDROID_HOME_VALUE/cmdline-tools/latest/bin:$RUNNER_HOME/.cargo/bin:$RUNNER_HOME/.local/bin:/usr/local/bin:/usr/bin:/bin"
)

sudo -u "$RUNNER_USER" -H env "${runner_env[@]}" \
  pnpm config set store-dir "$RUNNER_HOME/.cache/pnpm/store" --global

printf 'Installing frozen JavaScript dependencies for bootstrap verification...\n'
sudo -u "$RUNNER_USER" -H env "${runner_env[@]}" \
  bash -Eeuo pipefail -c 'cd "$1" && pnpm install --frozen-lockfile' _ "$BOOTSTRAP_SOURCE"

printf 'Installing repository-pinned Android SDK, NDK, emulator, and system image...\n'
sudo -u "$RUNNER_USER" -H env "${runner_env[@]}" \
  bash -Eeuo pipefail -c 'cd "$1" && bash ./scripts/install-android-deps.sh --with-emulator' _ "$BOOTSTRAP_SOURCE"

printf 'Creating the pinned Android AVD under the runner account...\n'
sudo -u "$RUNNER_USER" -H env "${runner_env[@]}" \
  bash -Eeuo pipefail -c 'cd "$1" && bash ./scripts/android-avd.sh create' _ "$BOOTSTRAP_SOURCE"

printf 'Provisioning the pinned Linux PDFium runtime for initial qualification...\n'
sudo -u "$RUNNER_USER" -H env "${runner_env[@]}" \
  bash -Eeuo pipefail -c 'cd "$1" && bash ./scripts/install-native-deps.sh --pdfium-only' _ "$BOOTSTRAP_SOURCE"

printf 'Verifying guest and nested KVM after toolchain installation...\n'
sudo -u "$RUNNER_USER" -H env "${runner_env[@]}" \
  bash -Eeuo pipefail -c 'cd "$1" && bash ./scripts/runner/guest-preflight.sh' _ "$BOOTSTRAP_SOURCE"

printf '\nOK   Unified runner guest bootstrap completed.\n'
printf 'The GitHub runner has not been downloaded or registered.\n'
printf 'Next: run the known-green Linux and Android qualification commands locally as %s.\n' "$RUNNER_USER"
printf 'After they pass, harden the administration account, take a powered-off pre-registration snapshot, and register the repository runner.\n'

#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"

log() { printf '\n\033[1;34m==> %s\033[0m\n' "$*"; }
die() { printf '\n\033[1;31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }

[[ "$(uname -s)" == "Linux" ]] || die "This setup script is for Ubuntu/Debian Linux only."
command -v apt-get >/dev/null 2>&1 || die "apt-get was not found."

log "Installing Ubuntu/Debian packages required by Tauri and the native smoke test"
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  build-essential \
  ca-certificates \
  curl \
  file \
  git \
  jq \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libssl-dev \
  libwebkit2gtk-4.1-dev \
  libxdo-dev \
  pkg-config \
  python3 \
  sqlite3 \
  tar \
  unzip \
  webkit2gtk-driver \
  wget \
  xauth \
  xvfb \
  xz-utils

LOCAL_BIN="$HOME/.local/bin"
LOCAL_OPT="$HOME/.local/opt"
mkdir -p "$LOCAL_BIN" "$LOCAL_OPT"
export PATH="$LOCAL_BIN:$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(nproc)}"

add_path_to_shell_file() {
  local file="$1"
  local marker="# local ostudy development tools"
  touch "$file"
  if ! grep -Fq "$marker" "$file"; then
    cat >> "$file" <<'PATH_SNIPPET'

# local ostudy development tools
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
PATH_SNIPPET
  fi
}

add_path_to_shell_file "$HOME/.profile"
if [[ -f "$HOME/.bashrc" || "${SHELL:-}" == */bash ]]; then
  add_path_to_shell_file "$HOME/.bashrc"
fi

case "$(uname -m)" in
  x86_64) NODE_ARCH="x64" ;;
  aarch64|arm64) NODE_ARCH="arm64" ;;
  *) die "Unsupported CPU architecture: $(uname -m)" ;;
esac

NODE_DIST="node-v${NODE_VERSION}-linux-${NODE_ARCH}"
NODE_INSTALL_DIR="$LOCAL_OPT/$NODE_DIST"
NODE_CURRENT_LINK="$LOCAL_OPT/node"

if [[ ! -x "$NODE_INSTALL_DIR/bin/node" ]]; then
  log "Installing Node.js ${NODE_VERSION} with checksum verification"
  TMP_DIR="$(mktemp -d)"
  trap 'rm -rf "${TMP_DIR:-}"' EXIT
  BASE_URL="https://nodejs.org/dist/v${NODE_VERSION}"
  curl --fail --location --retry 3 --output "$TMP_DIR/SHASUMS256.txt" "$BASE_URL/SHASUMS256.txt"
  curl --fail --location --retry 3 --output "$TMP_DIR/${NODE_DIST}.tar.xz" "$BASE_URL/${NODE_DIST}.tar.xz"
  (
    cd "$TMP_DIR"
    grep "  ${NODE_DIST}\\.tar\\.xz$" SHASUMS256.txt | sha256sum --check --strict -
  )
  tar -xJf "$TMP_DIR/${NODE_DIST}.tar.xz" -C "$LOCAL_OPT"
else
  log "Node.js ${NODE_VERSION} is already installed"
fi

ln -sfn "$NODE_INSTALL_DIR" "$NODE_CURRENT_LINK"
for executable in node npm npx corepack; do
  [[ -x "$NODE_CURRENT_LINK/bin/$executable" ]] && \
    ln -sfn "$NODE_CURRENT_LINK/bin/$executable" "$LOCAL_BIN/$executable"
done

actual_npm="$($NODE_CURRENT_LINK/bin/npm --version)"
[[ "$actual_npm" == "$NPM_VERSION" ]] || \
  die "Node ${NODE_VERSION} was expected to include npm ${NPM_VERSION}, found ${actual_npm}."

log "Installing pnpm ${PNPM_VERSION} into the user-local prefix"
"$NODE_CURRENT_LINK/bin/npm" install --global --prefix "$HOME/.local" "pnpm@${PNPM_VERSION}"

if ! command -v rustup >/dev/null 2>&1; then
  log "Installing rustup"
  curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
    https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
fi
# shellcheck disable=SC1091
source "$HOME/.cargo/env"

log "Installing Rust ${RUST_VERSION}, rustfmt, and Clippy"
rustup toolchain install "$RUST_VERSION" --profile minimal --component rustfmt --component clippy
rustup default "$RUST_VERSION"

installed_driver="$(
  cargo install --list |
    sed -n 's/^tauri-driver v\([^:]*\):$/\1/p' |
    head -n 1
)"
if [[ "$installed_driver" != "$TAURI_DRIVER_VERSION" ]]; then
  log "Installing tauri-driver ${TAURI_DRIVER_VERSION} with $CARGO_BUILD_JOBS parallel Cargo jobs"
  cargo install tauri-driver --version "$TAURI_DRIVER_VERSION" --locked --force
else
  log "tauri-driver ${TAURI_DRIVER_VERSION} is already installed"
fi

log "Building ostudy once so ordinary startup is fast"
"$SCRIPT_DIR/build.sh"

cat <<SETUP_DONE

Setup is complete.

The initial standalone application build used ${CARGO_BUILD_JOBS} Cargo jobs.
Cargo also reuses compiled dependencies on later builds.

Start the existing build quickly with:
  ./run-app.sh

After changing source code, rebuild with:
  ./scripts/build.sh

Before committing a change, run the complete one-command gate:
  ./scripts/check.sh
SETUP_DONE

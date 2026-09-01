#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/node-toolchain.bash"
ostudy_activate_node_toolchain

CHECK_NATIVE_SMOKE=0
case "${1:-}" in
  "") ;;
  --native-smoke) CHECK_NATIVE_SMOKE=1 ;;
  *)
    printf 'Usage: ./scripts/doctor.sh [--native-smoke]\n' >&2
    exit 2
    ;;
esac

failures=0
ok() { printf 'OK   %s\n' "$*"; }
fail() { printf 'FAIL %s\n' "$*" >&2; failures=$((failures + 1)); }

require_command() {
  local name="$1"
  if command -v "$name" >/dev/null 2>&1; then
    ok "$name: $(command -v "$name")"
  else
    fail "$name is missing"
  fi
}

exact_version() {
  local label="$1" actual="$2" expected="$3"
  if [[ "$actual" == "$expected" ]]; then
    ok "$label $actual"
  else
    fail "$label expected $expected, found $actual"
  fi
}

printf 'ostudy development environment diagnostics\n\n'

for command_name in \
  git curl jq node npm pnpm python3 rustup rustc cargo rustfmt cargo-clippy \
  pkg-config sqlite3 sha256sum tar; do
  require_command "$command_name"
done

if [[ -n "${OSTUDY_NODE_TOOLCHAIN_BIN:-}" ]]; then
  ok "Node toolchain: ${OSTUDY_NODE_TOOLCHAIN_SOURCE} (${OSTUDY_NODE_TOOLCHAIN_BIN})"
else
  printf 'NOTE No exact Node %s/npm %s pair was found in the project-managed install or PATH.\n' \
    "$NODE_VERSION" "$NPM_VERSION" >&2
  printf '     Run ./scripts/setup-ubuntu.sh or activate the repository .nvmrc, then retry.\n' >&2
fi

if (( CHECK_NATIVE_SMOKE == 1 )); then
  for command_name in xvfb-run openbox xprop WebKitWebDriver tauri-driver; do
    require_command "$command_name"
  done
fi

if (( failures == 0 )); then
  exact_version "Node" "$(node --version | sed 's/^v//')" "$NODE_VERSION"
  exact_version "npm" "$(npm --version)" "$NPM_VERSION"
  exact_version "pnpm" "$(pnpm --version)" "$PNPM_VERSION"
  exact_version "Rust" "$(rustc --version | awk '{print $2}')" "$RUST_VERSION"
  exact_version "Cargo" "$(cargo --version | awk '{print $2}')" "$CARGO_VERSION"

  if (( CHECK_NATIVE_SMOKE == 1 )); then
    tauri_driver_version="$(
      cargo install --list |
        sed -n 's/^tauri-driver v\([^:]*\):$/\1/p' |
        head -n 1
    )"
    exact_version \
      "tauri-driver" \
      "$tauri_driver_version" \
      "$TAURI_DRIVER_VERSION"
  fi

  if pkg-config --exists webkit2gtk-4.1; then
    ok "webkit2gtk-4.1 $(pkg-config --modversion webkit2gtk-4.1)"
  else
    fail "webkit2gtk-4.1 development files are missing"
  fi

  if pkg-config --exists openssl; then
    ok "OpenSSL development files $(pkg-config --modversion openssl)"
  else
    fail "OpenSSL development files are missing"
  fi
fi

if (( failures > 0 )); then
  printf '\nFAILED: %d environment check(s) did not pass.\n' "$failures" >&2
  exit 1
fi

if (( CHECK_NATIVE_SMOKE == 1 )); then
  printf '\nOK   Core and native-smoke requirements passed.\n'
else
  printf '\nOK   Core build requirements passed.\n'
fi

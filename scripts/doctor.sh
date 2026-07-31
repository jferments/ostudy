#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"

export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

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
  local label="$1"
  local actual="$2"
  local expected="$3"
  if [[ "$actual" == "$expected" ]]; then
    ok "$label $actual"
  else
    fail "$label expected $expected, found $actual"
  fi
}

printf 'Study App development environment diagnostics\n\n'

for command_name in git curl jq node npm pnpm rustup rustc cargo rustfmt cargo-clippy pkg-config sqlite3; do
  require_command "$command_name"
done

if (( failures == 0 )); then
  exact_version "Node" "$(node --version | sed 's/^v//')" "$NODE_VERSION"
  exact_version "npm" "$(npm --version)" "$NPM_VERSION"
  exact_version "pnpm" "$(pnpm --version)" "$PNPM_VERSION"
  exact_version "Rust" "$(rustc --version | awk '{print $2}')" "$RUST_VERSION"
  exact_version "Cargo" "$(cargo --version | awk '{print $2}')" "$CARGO_VERSION"

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

printf '\nOK   All exact toolchain and system checks passed.\n'

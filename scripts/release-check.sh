#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

[[ -f "$ROOT_DIR/pnpm-lock.yaml" ]] || {
  printf 'ERROR: pnpm-lock.yaml is missing. Generate and commit it before release qualification.\n' >&2
  exit 1
}
[[ -f "$ROOT_DIR/src-tauri/Cargo.lock" ]] || {
  printf 'ERROR: src-tauri/Cargo.lock is missing. Generate and commit it before release qualification.\n' >&2
  exit 1
}

TEMP_ROOT="$(mktemp -d -t study-app-release-check.XXXXXXXX)"
cleanup() { rm -rf -- "$TEMP_ROOT"; }
trap cleanup EXIT

printf '\n==> Creating a clean-room source copy\n'
mkdir -p "$TEMP_ROOT/project"
(
  cd "$ROOT_DIR"
  tar \
    --exclude='./.git' \
    --exclude='./node_modules' \
    --exclude='./dist' \
    --exclude='./src-tauri/target' \
    --exclude='./.study-app-backups' \
    --exclude='./.hotfix-backups' \
    --exclude='./.study-app-*-validated' \
    --exclude='./.release-check-*' \
    -cf - .
) | tar -xf - -C "$TEMP_ROOT/project"

printf '\n==> Running the full gate from committed lockfiles\n'
(
  cd "$TEMP_ROOT/project"
  ./scripts/check.sh
)

printf '\nClean-room release candidate checks passed.\n'

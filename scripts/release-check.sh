#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

[[ -f "$ROOT_DIR/pnpm-lock.yaml" ]] || {
  printf 'ERROR: pnpm-lock.yaml is missing.\n' >&2
  exit 1
}
[[ -f "$ROOT_DIR/src-tauri/Cargo.lock" ]] || {
  printf 'ERROR: src-tauri/Cargo.lock is missing.\n' >&2
  exit 1
}

TEMP_ROOT="$(mktemp -d -t ostudy-release-check.XXXXXXXX)"
cleanup() { rm -rf -- "$TEMP_ROOT"; }
trap cleanup EXIT

printf '\n==> Creating a clean source copy\n'
mkdir -p "$TEMP_ROOT/project"
(
  cd "$ROOT_DIR"
  tar \
    --exclude='./.git' \
    --exclude='./node_modules' \
    --exclude='./dist' \
    --exclude='./src-tauri/target' \
    --exclude='./artifacts' \
    --exclude='./.artifacts' \
    --exclude='./.ostudy-build-stamp' \
    --exclude='./*.tar.gz' \
    -cf - .
) | tar -xf - -C "$TEMP_ROOT/project"

# check.sh guards each step with git status, so the copy needs its own
# repository. One local commit of the copied tree is enough; nothing in it
# is pushed or tagged.
(
  cd "$TEMP_ROOT/project"
  git init -q
  git add -A
  git -c user.name='ostudy release check' -c user.email='release-check@localhost' \
    -c commit.gpgsign=false commit -q --no-verify -m 'Release candidate source copy'
)

printf '\n==> Running the complete gate in the clean source copy\n'
(
  cd "$TEMP_ROOT/project"
  ./scripts/check.sh
)

printf '\nClean-source release candidate checks passed.\n'

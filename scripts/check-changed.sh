#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/cargo-target.sh"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(nproc)}"
cd "$ROOT_DIR"

BASE="${1:-${OSTUDY_CHANGED_BASE:-}}"
if [[ -z "$BASE" ]]; then
  if git rev-parse --verify --quiet origin/main >/dev/null; then
    BASE="origin/main"
  elif git rev-parse --verify --quiet main >/dev/null; then
    BASE="main"
  else
    BASE="HEAD"
  fi
fi

mapfile -t CHANGED < <(
  {
    if [[ "$BASE" != "HEAD" ]] || git rev-parse --verify --quiet HEAD^ >/dev/null; then
      git diff --name-only --diff-filter=ACMRD "$BASE"...HEAD 2>/dev/null || true
    fi
    git diff --name-only --diff-filter=ACMRD
    git diff --cached --name-only --diff-filter=ACMRD
    git ls-files --others --exclude-standard
  } | sed '/^$/d' | sort -u
)

printf 'Changed-file developer check against %s\n' "$BASE"
printf 'This is an iteration aid, not a merge qualification gate.\n'

if [[ ${#CHANGED[@]} -eq 0 ]]; then
  printf 'No changed files detected.\n'
  exit 0
fi

printf 'Changed files: %s\n' "${#CHANGED[@]}"
printf '  %s\n' "${CHANGED[@]}"

is_docs_only=1
is_frontend_only=1
is_rust_only=1
document_only=1
requires_fast=0

for path in "${CHANGED[@]}"; do
  if [[ ! "$path" =~ ^(docs/|README\.md$|LICENSE$) ]]; then
    is_docs_only=0
  fi
  if [[ ! "$path" =~ ^src/.*\.(ts|tsx|css)$ ]]; then
    is_frontend_only=0
  fi
  if [[ ! "$path" =~ ^src-tauri/src/.*\.rs$ ]]; then
    is_rust_only=0
  fi
  if [[ ! "$path" =~ ^(src-tauri/src/(documents\.rs|document_.*\.rs|ai/document_.*\.rs)|src-tauri/migrations/00(17|18|19|20)_.*\.sql|src-tauri/tests/fixtures/document-generation/|scripts/(check-document\.sh|analyze-document-usage\.mjs)|docs/(AI-ARCHITECTURE\.md|decisions/002[0-9]-.*|development/.*document.*)) ]]; then
    document_only=0
  fi
  if [[ "$path" =~ ^(scripts/|\.github/|src-tauri/migrations/|package\.json$|pnpm-lock\.yaml$|pnpm-workspace\.yaml$|versions\.env$|src-tauri/Cargo\.toml$|src-tauri/Cargo\.lock$|src-tauri/tauri\.conf\.json$|src-tauri/build\.rs$) ]]; then
    requires_fast=1
  fi
done

if [[ "$is_docs_only" == "1" ]]; then
  printf '\nDocs-only change: no executable checks selected.\n'
  exit 0
fi

if [[ "$document_only" == "1" ]]; then
  printf '\nDocument-platform-only change detected; running focused cross-layer contracts.\n'
  bash "$SCRIPT_DIR/check-static.sh"
  prepare_cargo_target "$ROOT_DIR"
  cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features
  exec bash "$SCRIPT_DIR/check-document.sh"
fi

if [[ "$requires_fast" == "1" ]]; then
  printf '\nInfrastructure, manifest, or migration change detected; escalating to check-fast.sh.\n'
  exec bash "$SCRIPT_DIR/check-fast.sh"
fi

if [[ "$is_frontend_only" == "1" ]]; then
  bash "$SCRIPT_DIR/check-static.sh"
  printf '\n==> Frontend tests related to changed files\n'
  pnpm exec vitest run --changed "$BASE" --passWithNoTests
  exit 0
fi

if [[ "$is_rust_only" == "1" ]]; then
  bash "$SCRIPT_DIR/check-static.sh"
  prepare_cargo_target "$ROOT_DIR"
  printf '\n==> Rust compile check for the current tree\n'
  cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features
  printf '\n==> Rust library tests\n'
  cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib
  exit 0
fi

printf '\nMixed or cross-layer change detected; escalating to check-fast.sh.\n'
exec bash "$SCRIPT_DIR/check-fast.sh"

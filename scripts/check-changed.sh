#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/node-toolchain.bash"
ostudy_activate_node_toolchain
# shellcheck disable=SC1091
source "$SCRIPT_DIR/cargo-target.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/console-events.bash"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
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

ostudy_console_stage changed "Inspecting changed files"
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
  if [[ ! "$path" =~ ^(docs/|README\.md$|CONTRIBUTING\.md$|AGENTS\.md$|LICENSE$|\.github/ISSUE_TEMPLATE/) ]]; then
    is_docs_only=0
  fi
  if [[ ! "$path" =~ ^src/.*\.(ts|tsx|css)$ ]]; then
    is_frontend_only=0
  fi
  if [[ ! "$path" =~ ^src-tauri/src/.*\.rs$ ]]; then
    is_rust_only=0
  fi
  if [[ ! "$path" =~ ^(src-tauri/src/(documents\.rs|document_.*\.rs|pdf_.*\.rs|ai/document_.*\.rs)|src-tauri/migrations/00(17|18|19|20|21)_.*\.sql|src-tauri/tests/fixtures/document-generation/|scripts/analyze-document-usage\.mjs|docs/(architecture/ai-and-document-processing\.md|user-guide/documents-and-ai-generation\.md|decisions/002[0-9]-.*|development/.*document.*)) ]]; then
    document_only=0
  fi
  if [[ "$path" =~ ^(scripts/|\.github/|src-tauri/migrations/|package\.json$|pnpm-lock\.yaml$|pnpm-workspace\.yaml$|versions\.env$|src-tauri/Cargo\.toml$|src-tauri/Cargo\.lock$|src-tauri/tauri\.conf\.json$|src-tauri/build\.rs$) ]]; then
    requires_fast=1
  fi
done

if [[ "$is_docs_only" == "1" ]]; then
  ostudy_console_stage changed "Documentation verification"
  printf '\nDocs-only change: verifying documentation links and repository organization.\n'
  node "$SCRIPT_DIR/verify-documentation.mjs"
  exit 0
fi

if [[ "$document_only" == "1" ]]; then
  ostudy_console_stage changed "Document-platform contracts"
  printf '\nDocument-platform-only change detected; running focused cross-layer contracts.\n'
  bash "$SCRIPT_DIR/check-static.sh"
  prepare_cargo_target "$ROOT_DIR"
  cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features
  cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib document_
  cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib pdf_
  cargo test \
    --manifest-path src-tauri/Cargo.toml \
    --locked \
    --all-features \
    --lib \
    database::tests::migrations_and_repairs_are_idempotent_and_reach_latest_schema \
    -- --exact
  cargo test \
    --manifest-path src-tauri/Cargo.toml \
    --locked \
    --all-features \
    --lib \
    database::tests::released_schema_upgrades_forward_without_losing_integrity \
    -- --exact
  exit 0
fi

if [[ "$requires_fast" == "1" ]]; then
  ostudy_console_stage changed "Escalating to fast qualification"
  printf '\nInfrastructure, manifest, or migration change detected; escalating to check-fast.sh.\n'
  exec bash "$SCRIPT_DIR/check-fast.sh"
fi

if [[ "$is_frontend_only" == "1" ]]; then
  ostudy_console_stage changed "Frontend changed-file tests"
  bash "$SCRIPT_DIR/check-static.sh"
  printf '\n==> Frontend tests related to changed files\n'
  pnpm exec vitest run --changed "$BASE" --passWithNoTests
  exit 0
fi

if [[ "$is_rust_only" == "1" ]]; then
  ostudy_console_stage changed "Rust changed-file tests"
  bash "$SCRIPT_DIR/check-static.sh"
  prepare_cargo_target "$ROOT_DIR"
  printf '\n==> Rust compile check for the current tree\n'
  cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features
  printf '\n==> Rust library tests\n'
  cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib
  exit 0
fi

ostudy_console_stage changed "Escalating mixed change to fast qualification"
printf '\nMixed or cross-layer change detected; escalating to check-fast.sh.\n'
exec bash "$SCRIPT_DIR/check-fast.sh"

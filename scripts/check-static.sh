#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/check-state.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/qualification-gates.sh"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
cd "$ROOT_DIR"

CURRENT_STEP="initialization"
step() { CURRENT_STEP="$*"; printf '\n\033[1;34m==> %s\033[0m\n' "$CURRENT_STEP"; }
trap 'status=$?; printf "\n\033[1;31mSTATIC CHECK FAILED during: %s (exit %s)\033[0m\n" "$CURRENT_STEP" "$status" >&2; exit "$status"' ERR

if [[ "${OSTUDY_FORCE_CHECK:-0}" != "1" ]] \
  && ostudy_javascript_dependencies_present \
  && ostudy_check_stamp_is_current static; then
  printf '\n\033[1;32mREUSE Static qualification already passed for this exact source tree.\033[0m\n'
  exit 0
fi

step "Environment and repository contracts"
ostudy_gate_doctor
ostudy_gate_project_manifests
ostudy_gate_ui_token_generation
ostudy_gate_ui_token_usage
ostudy_gate_ipc_contract
ostudy_gate_repository_policy
ostudy_gate_platform_boundaries
ostudy_gate_external_file_streaming
ostudy_gate_documentation
ostudy_gate_lockfiles

step "Behavior-test policy and repository script validation"
ostudy_gate_test_policy
ostudy_gate_script_syntax
ostudy_gate_script_tests

step "Frozen JavaScript installation and dependency verification"
ostudy_gate_pnpm_install
ostudy_gate_js_versions

step "Frontend type checking"
ostudy_gate_typescript

step "Rust formatting"
ostudy_gate_rustfmt

ostudy_write_check_stamp static
trap - ERR
printf '\n\033[1;32mStatic ostudy checks passed.\033[0m\n'

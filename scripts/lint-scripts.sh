#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/script-linters.bash"
cd "$ROOT_DIR"

ostudy_ensure_script_linters

mapfile -d '' shell_files < <(
  git ls-files -z -- '*.sh' '*.bash'
)
if (( ${#shell_files[@]} == 0 )); then
  printf 'ERROR: no tracked shell sources were found for ShellCheck.\n' >&2
  exit 1
fi
"$OSTUDY_SHELLCHECK" \
  --severity=error \
  --external-sources \
  --source-path=SCRIPTDIR \
  "${shell_files[@]}"
printf 'OK   ShellCheck %s: %d tracked shell file(s)\n' \
  "$SHELLCHECK_VERSION" "${#shell_files[@]}"

mapfile -d '' python_files < <(
  git ls-files -z -- '*.py'
)
if (( ${#python_files[@]} == 0 )); then
  printf 'ERROR: no tracked Python sources were found for Ruff.\n' >&2
  exit 1
fi
"$OSTUDY_RUFF" check \
  --select E9,F63,F7,F82 \
  --output-format concise \
  "${python_files[@]}"
printf 'OK   Ruff %s correctness lint: %d tracked Python file(s)\n' \
  "$RUFF_VERSION" "${#python_files[@]}"

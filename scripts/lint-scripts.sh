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

TYPE_PROBE="$(mktemp -t ostudy-ty-negative.XXXXXXXX.py)"
cleanup_type_probe() { rm -f -- "$TYPE_PROBE"; }
trap cleanup_type_probe EXIT
cat >"$TYPE_PROBE" <<'PY'
value: int = "not an integer"
PY
if "$OSTUDY_TY" check --python-version 3.12 --no-progress "$TYPE_PROBE" >/dev/null 2>&1; then
  printf 'ERROR: ty negative control accepted an incompatible assignment.\n' >&2
  exit 1
fi
rm -f -- "$TYPE_PROBE"
trap - EXIT

"$OSTUDY_TY" check \
  --python-version 3.12 \
  --no-progress \
  --output-format concise \
  scripts/library-status.py \
  scripts/restore_pre_public_backup.py
printf 'OK   ty %s focused type check: library status + pre-public restore\n' "$TY_VERSION"

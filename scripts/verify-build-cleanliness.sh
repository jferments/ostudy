#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

changes="$(git status --porcelain --untracked-files=no)"
if [[ -n "$changes" ]]; then
  printf 'Build/test execution modified tracked source files:\n%s\n' "$changes" >&2
  printf '\nTracked diff left by build/test execution:\n' >&2
  git diff -- >&2 || true
  exit 1
fi
printf 'OK   Build/test execution left tracked source files clean.\n'

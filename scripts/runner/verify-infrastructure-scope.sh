#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

BASE_REF="${1:-${OSTUDY_INFRA_BASE_SHA:-}}"
HEAD_REF="${2:-${OSTUDY_INFRA_HEAD_SHA:-HEAD}}"

if [[ -z "$BASE_REF" ]]; then
  if git rev-parse --verify --quiet origin/main >/dev/null; then
    BASE_REF="$(git merge-base origin/main "$HEAD_REF")"
  elif git rev-parse --verify --quiet main >/dev/null; then
    BASE_REF="$(git merge-base main "$HEAD_REF")"
  else
    printf 'Unable to resolve an infrastructure-scope base. Pass BASE_REF explicitly.\n' >&2
    exit 2
  fi
fi

git rev-parse --verify "$BASE_REF^{commit}" >/dev/null
git rev-parse --verify "$HEAD_REF^{commit}" >/dev/null

mapfile -t changed_paths < <(git diff --name-only "$BASE_REF...$HEAD_REF")
if ((${#changed_paths[@]} == 0)); then
  printf 'OK   No changed paths between %s and %s.\n' "$BASE_REF" "$HEAD_REF"
  exit 0
fi

printf 'Infrastructure scope from %s to %s:\n' "$BASE_REF" "$HEAD_REF"
printf '  %s\n' "${changed_paths[@]}"

violations=()
for path in "${changed_paths[@]}"; do
  case "$path" in
    .github/workflows/*.yml|.github/workflows/*.yaml)
      ;;
    docs/README.md|docs/development/*.md)
      ;;
    scripts/runner/*|scripts/verify-project-manifests.mjs)
      ;;
    *)
      violations+=("$path")
      ;;
  esac
done

if ((${#violations[@]} > 0)); then
  printf '\nInfrastructure-only scope violation. These paths are not allowed in this PR:\n' >&2
  printf '  %s\n' "${violations[@]}" >&2
  printf '\nAllowed paths are CI workflow YAML, runner/development documentation, scripts/runner/*, and the project-manifest CI policy verifier.\n' >&2
  printf 'Application, Android app/test, plugin, migration, dependency-manifest, and product-source changes require a separate reviewed task.\n' >&2
  exit 1
fi

printf '\nOK   Every changed path is confined to runner/CI infrastructure and its documentation/policy contract.\n'

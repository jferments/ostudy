#!/usr/bin/env bash
set -euo pipefail

if (( $# < 2 )); then
  echo "usage: persist-runner-output.sh <bucket> <path> [path ...]" >&2
  exit 2
fi

bucket="$1"
shift

root="${OSTUDY_RUNNER_OUTPUT_ROOT:-$HOME/.local/state/ostudy/actions}"
run_id="${GITHUB_RUN_ID:-local}"
attempt="${GITHUB_RUN_ATTEMPT:-1}"
destination="$root/${run_id}-${attempt}/$bucket"
mkdir -p "$destination"

copied=0
for source in "$@"; do
  if [[ -e "$source" ]]; then
    cp -a -- "$source" "$destination/"
    copied=$((copied + 1))
  fi
done

if (( copied == 0 )); then
  echo "No runner-local output was available for bucket '$bucket'."
  exit 0
fi

{
  printf 'repository=%s\n' "${GITHUB_REPOSITORY:-local}"
  printf 'runId=%s\n' "$run_id"
  printf 'runAttempt=%s\n' "$attempt"
  printf 'sha=%s\n' "${GITHUB_SHA:-unknown}"
  printf 'bucket=%s\n' "$bucket"
  printf 'createdAt=%s\n' "$(date --utc --iso-8601=seconds)"
} > "$destination/manifest.txt"

size="$(du -sh "$destination" | awk '{print $1}')"
echo "Persisted $bucket evidence on self-hosted runner: $destination ($size)"
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    echo
    echo "### Runner-local evidence"
    echo
    echo "- Bucket: \`$bucket\`"
    echo "- Path: \`$destination\`"
    echo "- Size: **$size**"
    echo "- Cloud artifact upload: **disabled**"
  } >> "$GITHUB_STEP_SUMMARY"
fi

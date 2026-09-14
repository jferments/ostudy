#!/usr/bin/env bash
set -euo pipefail

if (( $# < 2 )); then
  echo "usage: persist-runner-output.sh <bucket> <path> [path ...]" >&2
  exit 2
fi

bucket="$1"
shift

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
root="${OSTUDY_RUNNER_OUTPUT_ROOT:-$HOME/.local/state/ostudy/actions}"
retention_days="${OSTUDY_RUNNER_OUTPUT_RETENTION_DAYS:-7}"
max_bytes="${OSTUDY_RUNNER_OUTPUT_MAX_BYTES:-21474836480}"
run_id="${GITHUB_RUN_ID:-local}"
attempt="${GITHUB_RUN_ATTEMPT:-1}"
run_key="${run_id}-${attempt}"
destination="$root/$run_key/$bucket"
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

python3 "$script_dir/prune-runner-output.py" \
  "$root" \
  --current "$run_key" \
  --retention-days "$retention_days" \
  --max-bytes "$max_bytes"

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
    echo "- Local retention: **${retention_days} days / 20 GiB default ceiling**"
    echo "- Cloud artifact upload: **disabled**"
  } >> "$GITHUB_STEP_SUMMARY"
fi

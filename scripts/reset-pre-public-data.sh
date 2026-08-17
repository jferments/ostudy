#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
  cat <<'EOF'
Usage: ./scripts/reset-pre-public-data.sh --data-dir <directory> --confirm

Moves the entire pre-release ostudy application-data directory to a timestamped
sibling backup and recreates an empty directory. It never deletes the old data.
Stop ostudy before running this command.
EOF
}

DATA_DIR=""
CONFIRMED=0
while (( $# > 0 )); do
  case "$1" in
    --data-dir) DATA_DIR="${2:-}"; shift ;;
    --confirm) CONFIRMED=1 ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown option: %s
' "$1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

[[ -n "$DATA_DIR" && "$DATA_DIR" = /* ]] || {
  printf 'An absolute --data-dir path is required.
' >&2
  exit 2
}
(( CONFIRMED == 1 )) || {
  printf 'Pass --confirm after reviewing the data directory.
' >&2
  exit 2
}
if pgrep -x ostudy >/dev/null 2>&1; then
  printf 'ostudy is still running. Close it before resetting pre-release data.
' >&2
  exit 1
fi
if [[ ! -d "$DATA_DIR" ]]; then
  mkdir -p "$DATA_DIR"
  printf 'Created empty ostudy data directory: %s
' "$DATA_DIR"
  exit 0
fi

parent="$(dirname -- "$DATA_DIR")"
name="$(basename -- "$DATA_DIR")"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
backup="$parent/$name.pre-public-$timestamp"
[[ ! -e "$backup" ]] || {
  printf 'Backup destination already exists: %s
' "$backup" >&2
  exit 1
}
mv -- "$DATA_DIR" "$backup"
mkdir -p -- "$DATA_DIR"
printf 'Moved pre-release ostudy data to: %s
' "$backup"
printf 'Created empty public-baseline data directory: %s
' "$DATA_DIR"

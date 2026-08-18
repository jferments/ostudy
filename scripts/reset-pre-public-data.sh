#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
  cat <<'EOF'
Usage:
  ./scripts/reset-pre-public-data.sh \
    --data-dir <directory> \
    --confirm-empty-library

INTENTIONALLY archives the entire active ostudy application-data directory and
creates an empty public-baseline directory. It never deletes the old data, but
the archived decks, cards, reviews, analytics, documents, and settings will not
appear in the new empty library.

The exact final pre-public v26 database is now adopted automatically on normal
startup. If an earlier reset already created a timestamped backup, restore it
with scripts/restore_pre_public_backup.py instead of using this command again.

Stop ostudy before running this command.
EOF
}

DATA_DIR=""
CONFIRMED=0
while (( $# > 0 )); do
  case "$1" in
    --data-dir) DATA_DIR="${2:-}"; shift ;;
    --confirm-empty-library) CONFIRMED=1 ;;
    --confirm)
      printf '%s\n' \
        'The old --confirm option is no longer accepted because it did not make data replacement explicit.' \
        'Use normal startup to preserve an exact final pre-public v26 library.' \
        'Use --confirm-empty-library only when you intentionally want a blank library.' >&2
      exit 2
      ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

[[ -n "$DATA_DIR" && "$DATA_DIR" = /* ]] || {
  printf 'An absolute --data-dir path is required.\n' >&2
  exit 2
}
(( CONFIRMED == 1 )) || {
  printf '%s\n' \
    'This command starts an EMPTY library.' \
    'Pass --confirm-empty-library only after reviewing the data directory.' >&2
  exit 2
}
if pgrep -x ostudy >/dev/null 2>&1; then
  printf 'ostudy is still running. Close it before archiving pre-release data.\n' >&2
  exit 1
fi
if [[ ! -d "$DATA_DIR" ]]; then
  mkdir -p "$DATA_DIR"
  printf 'Created empty ostudy data directory: %s\n' "$DATA_DIR"
  exit 0
fi

parent="$(dirname -- "$DATA_DIR")"
name="$(basename -- "$DATA_DIR")"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
backup="$parent/$name.pre-public-$timestamp"
[[ ! -e "$backup" ]] || {
  printf 'Backup destination already exists: %s\n' "$backup" >&2
  exit 1
}
mv -- "$DATA_DIR" "$backup"
mkdir -p -- "$DATA_DIR"
printf 'Archived the complete pre-release ostudy data directory at: %s\n' "$backup"
printf 'Created an intentionally empty public-baseline data directory: %s\n' "$DATA_DIR"
printf 'Restore the archive with scripts/restore_pre_public_backup.py if this was not intended.\n'
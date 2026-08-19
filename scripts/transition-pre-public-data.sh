#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage:
  bash ./scripts/transition-pre-public-data.sh \
    --data-dir <active-ostudy-data-directory> \
    --source-backup <timestamped-pre-public-backup-directory> \
    --confirm

Recovers an archived final pre-public ostudy library without deleting either
copy. The archived source is copied into staging, validated there, and installed
as the active app-data directory. The previous active directory is archived.
Normal ostudy startup then performs the verified final-v26 -> ostudy-public-v1
adoption in place while preserving learner data.

The original timestamped source backup is never opened by SQLite and is retained
unchanged.
EOF
}

DATA_DIR=""
SOURCE_BACKUP=""
CONFIRMED=0

while (( $# > 0 )); do
  case "$1" in
    --data-dir)
      [[ $# -ge 2 ]] || { printf 'Missing value for --data-dir.\n' >&2; exit 2; }
      DATA_DIR="$2"
      shift 2
      ;;
    --source-backup)
      [[ $# -ge 2 ]] || { printf 'Missing value for --source-backup.\n' >&2; exit 2; }
      SOURCE_BACKUP="$2"
      shift 2
      ;;
    --confirm)
      CONFIRMED=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'Unknown option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ -n "$DATA_DIR" && "$DATA_DIR" = /* ]] || {
  printf 'An absolute --data-dir path is required.\n' >&2
  exit 2
}
[[ -n "$SOURCE_BACKUP" && "$SOURCE_BACKUP" = /* ]] || {
  printf 'An absolute --source-backup path is required.\n' >&2
  exit 2
}
[[ "$DATA_DIR" != "$SOURCE_BACKUP" ]] || {
  printf -- '--data-dir and --source-backup must identify different directories.\n' >&2
  exit 2
}
(( CONFIRMED == 1 )) || {
  printf 'Pass --confirm after reviewing the active and backup directories.\n' >&2
  exit 2
}
command -v python3 >/dev/null 2>&1 || {
  printf 'python3 is required for the verified pre-public recovery transition.\n' >&2
  exit 1
}

printf 'Recovering archived pre-public library from: %s\n' "$SOURCE_BACKUP"
printf 'Target active ostudy data directory: %s\n' "$DATA_DIR"

exec python3 "$SCRIPT_DIR/restore_pre_public_backup.py" \
  --backup-dir "$SOURCE_BACKUP" \
  --data-dir "$DATA_DIR" \
  --confirm

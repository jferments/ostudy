#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage:
  bash ./scripts/transition-pre-public-data.sh \
    --data-dir <active-ostudy-data-directory> \
    [--source-backup <timestamped-pre-public-backup-directory>] \
    --confirm

Explicitly recovers the exact final pre-public-v26 ostudy lineage without
putting historical conversion code back into normal application startup.

Without --source-backup, the current --data-dir is first copied to an immutable
sibling archive, then a separate staging copy is validated and converted to the
ostudy-public-v1 baseline. With --source-backup, that archive is the immutable
source instead. The prior active directory is archived rather than deleted.

Current ostudy startup then applies only the ordinary public migration chain.
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
if [[ -n "$SOURCE_BACKUP" && "$SOURCE_BACKUP" != /* ]]; then
  printf 'When provided, --source-backup must be an absolute path.\n' >&2
  exit 2
fi
if [[ -z "$SOURCE_BACKUP" ]]; then
  SOURCE_BACKUP="$DATA_DIR"
fi
(( CONFIRMED == 1 )) || {
  printf 'Pass --confirm after reviewing the active and backup directories.\n' >&2
  exit 2
}
command -v python3 >/dev/null 2>&1 || {
  printf 'python3 is required for the verified pre-public recovery transition.\n' >&2
  exit 1
}

printf 'Explicit pre-public recovery source: %s\n' "$SOURCE_BACKUP"
printf 'Target active ostudy data directory: %s\n' "$DATA_DIR"

exec python3 "$SCRIPT_DIR/restore_pre_public_backup.py" \
  --backup-dir "$SOURCE_BACKUP" \
  --data-dir "$DATA_DIR" \
  --confirm

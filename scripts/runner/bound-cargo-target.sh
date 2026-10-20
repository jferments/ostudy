#!/usr/bin/env bash
set -Eeuo pipefail

# Keeps the persistent CI Cargo target from filling the runner's root disk.
# Incremental state from earlier runs never helps a fresh checkout's build, so
# it always goes before the job builds. The whole target is cleared when it outgrows
# its cap or when the disk is too full for a build to fit before the guest
# preflight minimum; the next run then builds cold.

target_dir="${OSTUDY_CARGO_TARGET_DIR:-}"
max_gib="${OSTUDY_CARGO_TARGET_MAX_GIB:-40}"
min_free_gib="${OSTUDY_CARGO_TARGET_MIN_FREE_GIB:-35}"
du_timeout_seconds="${OSTUDY_CARGO_TARGET_DU_TIMEOUT_SECONDS:-120}"

for setting in max_gib min_free_gib du_timeout_seconds; do
  if [[ ! "${!setting}" =~ ^[0-9]+$ ]]; then
    printf 'bound-cargo-target: %s must be a non-negative integer.\n' "$setting" >&2
    exit 2
  fi
done

# Only ever delete a dedicated cache directory named cargo-target.
if [[ "$target_dir" != /* || "$(basename -- "$target_dir")" != "cargo-target" ]]; then
  printf 'bound-cargo-target: refusing to manage %q; expected an absolute .../cargo-target path.\n' \
    "$target_dir" >&2
  exit 2
fi

mkdir -p -- "$target_dir"

removed_incremental=0
while IFS= read -r -d '' incremental_dir; do
  rm -rf -- "$incremental_dir"
  removed_incremental=$((removed_incremental + 1))
done < <(find "$target_dir" -mindepth 2 -maxdepth 3 -type d -name incremental -print0 2>/dev/null)
printf 'CARGO_TARGET removed %d incremental director%s\n' \
  "$removed_incremental" "$([[ "$removed_incremental" == 1 ]] && printf 'y' || printf 'ies')"

size_gib="$(
  timeout "$du_timeout_seconds" du -sxB1G -- "$target_dir" 2>/dev/null | awk '{print $1}' || true
)"
free_gib="$(df -PBG -- "$target_dir" | awk 'NR == 2 { sub(/G$/, "", $4); print $4 }')"

reason=""
if [[ -n "$size_gib" && "$size_gib" -gt "$max_gib" ]]; then
  reason="target is ${size_gib} GiB, above the ${max_gib} GiB cap"
elif [[ "$free_gib" -lt "$min_free_gib" ]]; then
  reason="root filesystem has ${free_gib} GiB free, below the ${min_free_gib} GiB build margin"
fi

if [[ -n "$reason" ]]; then
  printf 'CARGO_TARGET clearing %s: %s\n' "$target_dir" "$reason"
  rm -rf -- "$target_dir"
  mkdir -p -- "$target_dir"
  size_gib=0
  free_gib="$(df -PBG -- "$target_dir" | awk 'NR == 2 { sub(/G$/, "", $4); print $4 }')"
fi

printf 'CARGO_TARGET size=%s GiB cap=%s GiB free=%s GiB margin=%s GiB\n' \
  "${size_gib:-unknown}" "$max_gib" "$free_gib" "$min_free_gib"

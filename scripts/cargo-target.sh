#!/usr/bin/env bash

prepare_cargo_target() {
  local root_dir="$1"
  local target_dir="$root_dir/src-tauri/target"
  local marker="$target_dir/.ostudy-source-root"
  local recorded_root=""

  if [[ -f "$marker" ]]; then
    recorded_root="$(cat "$marker")"
  fi

  if [[ -d "$target_dir" && -n "$(find "$target_dir" -mindepth 1 -maxdepth 1 -print -quit 2>/dev/null)" ]]; then
    if [[ -z "$recorded_root" ]]; then
      printf '\n==> Clearing the pre-Phase-1 Cargo target once so it cannot retain stale absolute paths\n'
      rm -rf -- "$target_dir"
    elif [[ "$recorded_root" != "$root_dir" ]]; then
      printf '\n==> Repository moved from %s to %s; clearing stale Cargo artifacts\n' \
        "$recorded_root" "$root_dir"
      rm -rf -- "$target_dir"
    fi
  fi

  mkdir -p "$target_dir"
  printf '%s\n' "$root_dir" > "$marker"
}

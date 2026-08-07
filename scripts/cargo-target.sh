#!/usr/bin/env bash

ostudy_default_cargo_target_dir() {
  local root_dir="$1"
  if [[ -n "${OSTUDY_CARGO_TARGET_DIR:-}" ]]; then
    printf '%s\n' "$OSTUDY_CARGO_TARGET_DIR"
  elif [[ "${CI:-false}" == "true" ]]; then
    printf '%s/ostudy/cargo-target\n' "${XDG_CACHE_HOME:-$HOME/.cache}"
  else
    printf '%s/src-tauri/target\n' "$root_dir"
  fi
}

prepare_cargo_target() {
  local root_dir="$1"
  local target_dir="${CARGO_TARGET_DIR:-$(ostudy_default_cargo_target_dir "$root_dir")}"
  if [[ "$target_dir" != /* ]]; then
    target_dir="$root_dir/$target_dir"
  fi
  export CARGO_TARGET_DIR="$target_dir"

  case "$target_dir/" in
    "$root_dir/"*)
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
      ;;
    *)
      # CI uses a target outside the checkout. GitHub can safely clean the
      # workspace while Cargo retains dependency and build-script artifacts.
      # Cargo's own fingerprints invalidate stale compiler/features/source data.
      mkdir -p "$target_dir"
      ;;
  esac
}

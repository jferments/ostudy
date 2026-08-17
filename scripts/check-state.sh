#!/usr/bin/env bash

OSTUDY_ROOT_DIR="${OSTUDY_ROOT_DIR:-$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)}"

ostudy_check_state_key() {
  printf '%s' "$OSTUDY_ROOT_DIR" | sha256sum | awk '{print substr($1, 1, 16)}'
}

OSTUDY_CHECK_STATE_DIR="${OSTUDY_CHECK_STATE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/ostudy/check-state/$(ostudy_check_state_key)}"

ostudy_capture_worktree_state() {
  local destination="$1"
  (
    cd "$OSTUDY_ROOT_DIR"
    git status --porcelain=v1 --untracked-files=all --ignore-submodules=none
  ) > "$destination"
}

ostudy_assert_worktree_unchanged() {
  local baseline="$1"
  local boundary="${2:-qualification boundary}"
  local current
  current="$(mktemp -t ostudy-worktree-current.XXXXXXXX)"
  ostudy_capture_worktree_state "$current"
  if cmp -s -- "$baseline" "$current"; then
    rm -f -- "$current"
    return 0
  fi

  printf 'ERROR: %s changed Git-visible repository state.\n' "$boundary" >&2
  printf 'Qualification must not leave tracked changes or new untracked files behind.\n' >&2
  printf '%s\n' '--- worktree state before boundary' >&2
  printf '%s\n' '+++ worktree state after boundary' >&2
  diff -u --label before --label after "$baseline" "$current" >&2 || true
  rm -f -- "$current"
  return 1
}

ostudy_reset_worktree_baseline() {
  local baseline="$1"
  ostudy_capture_worktree_state "$baseline"
}

ostudy_check_fingerprint() {
  (
    cd "$OSTUDY_ROOT_DIR"
    {
      git ls-files -z
      git ls-files --others --exclude-standard -z
    } | sort -zu | while IFS= read -r -d '' path; do
      if [[ -f "$path" ]]; then
        sha256sum "$path"
      else
        printf 'missing  %s\n' "$path"
      fi
    done

    printf 'node='; command -v node >/dev/null 2>&1 && node --version || printf 'missing\n'
    printf 'pnpm='; command -v pnpm >/dev/null 2>&1 && pnpm --version || printf 'missing\n'
    printf 'rustc='; command -v rustc >/dev/null 2>&1 && rustc -Vv || printf 'missing\n'
    printf 'cargo='; command -v cargo >/dev/null 2>&1 && cargo --version || printf 'missing\n'
    printf 'platform='; uname -srm
  ) | sha256sum | awk '{print $1}'
}

ostudy_check_stamp_path() {
  local tier="$1"
  printf '%s/%s.stamp\n' "$OSTUDY_CHECK_STATE_DIR" "$tier"
}

ostudy_check_stamp_is_current() {
  local tier="$1"
  local stamp
  stamp="$(ostudy_check_stamp_path "$tier")"
  [[ -f "$stamp" ]] || return 1

  local recorded current
  recorded="$(sed -n 's/^fingerprint=//p' "$stamp" | head -n 1)"
  [[ -n "$recorded" ]] || return 1
  current="$(ostudy_check_fingerprint)"
  [[ "$recorded" == "$current" ]]
}

ostudy_write_check_stamp() {
  local tier="$1"
  local fingerprint="${2:-$(ostudy_check_fingerprint)}"
  mkdir -p "$OSTUDY_CHECK_STATE_DIR"
  cat > "$(ostudy_check_stamp_path "$tier")" <<STAMP
root=$OSTUDY_ROOT_DIR
tier=$tier
fingerprint=$fingerprint
STAMP
}

ostudy_javascript_dependencies_present() {
  [[ -d "$OSTUDY_ROOT_DIR/node_modules/.pnpm" && -x "$OSTUDY_ROOT_DIR/node_modules/.bin/tsc" ]]
}

ostudy_local_fonts_present() {
  command -v node >/dev/null 2>&1 || return 1
  node "$OSTUDY_ROOT_DIR/scripts/verify-local-fonts.mjs" --installed >/dev/null 2>&1
}

ostudy_frontend_prerequisites_present() {
  ostudy_javascript_dependencies_present && ostudy_local_fonts_present
}

ostudy_clear_check_stamps() {
  rm -f -- \
    "$(ostudy_check_stamp_path static)" \
    "$(ostudy_check_stamp_path fast)" \
    "$(ostudy_check_stamp_path full)"
}

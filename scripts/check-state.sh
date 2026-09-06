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

ostudy_report_worktree_baseline() {
  local baseline="$1"
  if [[ "${OSTUDY_WORKTREE_BASELINE_REPORTED:-0}" == 1 || ! -s "$baseline" ]]; then
    return 0
  fi
  printf 'NOTE: qualification starts with pre-existing Git-visible worktree state:\n' >&2
  sed 's/^/  /' "$baseline" >&2
  printf 'The mutation guard will preserve this exact baseline and fail on additional changes.\n' >&2
  export OSTUDY_WORKTREE_BASELINE_REPORTED=1
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
  ostudy_report_worktree_baseline "$baseline"
}

ostudy_qualification_scope() {
  local tier="${1:-exact}"
  case "$tier" in
    full|android-compile|android-smoke)
      printf 'product\n'
      ;;
    *)
      printf 'exact\n'
      ;;
  esac
}

ostudy_qualification_neutral_path() {
  local path="$1"
  [[ "$path" == *.md ]] || return 1
  [[ "$path" != */* || "$path" == docs/* ]]
}

ostudy_check_fingerprint() {
  local tier="${1:-exact}"
  local scope
  scope="$(ostudy_qualification_scope "$tier")"

  (
    cd "$OSTUDY_ROOT_DIR"
    printf 'scope=%s\n' "$scope"
    {
      git ls-files -z
      git ls-files --others --exclude-standard -z
    } | sort -zu | while IFS= read -r -d '' path; do
      if [[ "$scope" == product ]] && ostudy_qualification_neutral_path "$path"; then
        continue
      fi
      if [[ -f "$path" ]]; then
        sha256sum "$path"
      else
        printf 'missing  %s\n' "$path"
      fi
    done

    printf 'node='; command -v node >/dev/null 2>&1 && node --version || printf 'missing\n'
    printf 'npm='; command -v npm >/dev/null 2>&1 && npm --version || printf 'missing\n'
    printf 'pnpm='; command -v pnpm >/dev/null 2>&1 && pnpm --version || printf 'missing\n'
    printf 'rustc='; command -v rustc >/dev/null 2>&1 && rustc -Vv || printf 'missing\n'
    printf 'cargo='; command -v cargo >/dev/null 2>&1 && cargo --version || printf 'missing\n'
    printf 'platform='; uname -srm

    if [[ "$tier" == android-* ]]; then
      printf 'java='; command -v java >/dev/null 2>&1 && java -version 2>&1 || printf 'missing\n'
      printf 'javac='; command -v javac >/dev/null 2>&1 && javac -version 2>&1 || printf 'missing\n'
      printf 'android_home=%s\n' "${ANDROID_HOME:-missing}"
      if [[ -n "${ANDROID_HOME:-}" && -x "${ANDROID_HOME}/platform-tools/adb" ]]; then
        printf 'adb='
        "${ANDROID_HOME}/platform-tools/adb" version 2>&1
      else
        printf 'adb=missing\n'
      fi
      if [[ -n "${ANDROID_HOME:-}" && -x "${ANDROID_HOME}/emulator/emulator" ]]; then
        printf 'emulator='
        "${ANDROID_HOME}/emulator/emulator" -version 2>&1
      else
        printf 'emulator=missing\n'
      fi
    fi
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
  current="$(ostudy_check_fingerprint "$tier")"
  [[ "$recorded" == "$current" ]]
}

ostudy_check_stamp_source_sha() {
  local tier="$1"
  local stamp
  stamp="$(ostudy_check_stamp_path "$tier")"
  [[ -f "$stamp" ]] || return 1
  sed -n 's/^source_sha=//p' "$stamp" | head -n 1
}

ostudy_write_check_stamp() {
  local tier="$1"
  local fingerprint="${2:-$(ostudy_check_fingerprint "$tier")}"
  local source_sha="unrecorded"
  if git -C "$OSTUDY_ROOT_DIR" rev-parse --verify HEAD >/dev/null 2>&1; then
    source_sha="$(git -C "$OSTUDY_ROOT_DIR" rev-parse HEAD)"
  fi
  mkdir -p "$OSTUDY_CHECK_STATE_DIR"
  cat > "$(ostudy_check_stamp_path "$tier")" <<STAMP
root=$OSTUDY_ROOT_DIR
tier=$tier
scope=$(ostudy_qualification_scope "$tier")
source_sha=$source_sha
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
    "$(ostudy_check_stamp_path full)" \
    "$(ostudy_check_stamp_path android-compile)" \
    "$(ostudy_check_stamp_path android-smoke)"
}

#!/usr/bin/env bash

# Keep disposable CI fixtures off the qcow2 root filesystem when the isolated
# runner has enough guest RAM. Persistent compiler/package caches stay on disk.
OSTUDY_OWNED_CI_TEST_TMPDIR=""

ostudy_prepare_ci_test_tmpdir() {
  local scope="${1:-qualification}"
  [[ "${CI:-false}" == true ]] || return 0
  [[ "${OSTUDY_CI_USE_TMPFS:-1}" == 1 ]] || return 0

  local root="${OSTUDY_CI_TMPFS_ROOT:-/dev/shm}"
  [[ -d "$root" && -w "$root" ]] || {
    printf 'TMPFS CI temp unavailable at %s; using %s.\n' "$root" "${TMPDIR:-/tmp}"
    return 0
  }

  local free_kib
  free_kib="$(df -Pk "$root" | awk 'NR == 2 {print $4}')"
  if [[ ! "$free_kib" =~ ^[0-9]+$ ]] || (( free_kib < 4 * 1024 * 1024 )); then
    printf 'TMPFS CI temp has insufficient free space at %s; using %s.\n' "$root" "${TMPDIR:-/tmp}"
    return 0
  fi

  local run_key
  run_key="${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-1}-${BASHPID:-$$}"
  OSTUDY_OWNED_CI_TEST_TMPDIR="$(mktemp -d -p "$root" "ostudy-$scope-$run_key.XXXXXXXX")"
  chmod 700 "$OSTUDY_OWNED_CI_TEST_TMPDIR"
  export TMPDIR="$OSTUDY_OWNED_CI_TEST_TMPDIR"
  printf 'CI test temp: %s (tmpfs, %s MiB free before qualification).\n' \
    "$TMPDIR" "$((free_kib / 1024))"
}

ostudy_cleanup_ci_test_tmpdir() {
  if [[ -n "${OSTUDY_OWNED_CI_TEST_TMPDIR:-}" ]]; then
    rm -rf -- "$OSTUDY_OWNED_CI_TEST_TMPDIR"
    OSTUDY_OWNED_CI_TEST_TMPDIR=""
  fi
}

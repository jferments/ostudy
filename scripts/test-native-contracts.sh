#!/usr/bin/env bash
# Run selected native contracts, refusing empty or ignored-only test selections.
set -Eeuo pipefail
ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
if (( $# == 0 )); then
  printf 'Usage: scripts/test-native-contracts.sh <test-filter> [test-filter ...]\n' >&2
  exit 2
fi
TEMP_DIR="$(mktemp -d -t ostudy-native-contracts.XXXXXXXX)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT
for test_filter in "$@"; do
  if [[ -z "$test_filter" || "$test_filter" == -* ]]; then
    printf 'ERROR: a nonempty native test filter is required.\n' >&2
    exit 2
  fi
  args=(test --manifest-path "$ROOT_DIR/src-tauri/Cargo.toml" --locked --all-features --lib "$test_filter")
  cargo "${args[@]}" -- --list --color never > "$TEMP_DIR/list"
  count="$(awk '/: test$/ { n++ } END { print n+0 }' "$TEMP_DIR/list")"
  if (( count == 0 )); then
    printf 'ERROR: native contract filter matched no tests: %s\n' "$test_filter" >&2
    exit 1
  fi
  printf '\n==> Native contract %s: %s selected tests\n' "$test_filter" "$count"
  if [[ "${OSTUDY_NATIVE_CONTRACTS_LIST_ONLY:-0}" == "1" ]]; then
    # check.sh runs the whole native suite next, which runs these tests. Here
    # only refuse a selection that has gone empty or has ignored tests in it.
    cargo "${args[@]}" -- --list --ignored --color never > "$TEMP_DIR/ignored"
    ignored="$(awk '/: test$/ { n++ } END { print n+0 }' "$TEMP_DIR/ignored")"
    if (( ignored > 0 )); then
      printf 'ERROR: native contract has %s ignored tests: %s\n' "$ignored" "$test_filter" >&2
      exit 1
    fi
    printf 'LISTED Native contract %s: %s selected, none ignored; the full native suite runs them\n' "$test_filter" "$count"
    continue
  fi
  cargo "${args[@]}" -- --color never | tee "$TEMP_DIR/result"
  if ! grep -Eq "^test result: ok\\. ${count} passed; 0 failed; 0 ignored;" "$TEMP_DIR/result"; then
    printf 'ERROR: native contract did not execute every selected test: %s\n' "$test_filter" >&2
    exit 1
  fi
  printf 'PASS Native contract %s: %s executed, none ignored\n' "$test_filter" "$count"
done

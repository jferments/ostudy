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
  cargo "${args[@]}" -- --color never | tee "$TEMP_DIR/result"
  if ! grep -Eq "^test result: ok\\. ${count} passed; 0 failed; 0 ignored;" "$TEMP_DIR/result"; then
    printf 'ERROR: native contract did not execute every selected test: %s\n' "$test_filter" >&2
    exit 1
  fi
  printf 'PASS Native contract %s: %s executed, none ignored\n' "$test_filter" "$count"
done

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
index=0
for test_filter in "$@"; do
  if [[ -z "$test_filter" || "$test_filter" == -* ]]; then
    printf 'ERROR: a nonempty native test filter is required.\n' >&2
    exit 2
  fi
  args=(test --manifest-path "$ROOT_DIR/src-tauri/Cargo.toml" --locked --all-features --lib "$test_filter")
  result="$TEMP_DIR/result-$index"
  index=$((index + 1))
  printf '\n==> Native contract %s\n' "$test_filter"
  cargo "${args[@]}" -- --color never | tee "$result"

  counts="$(
    sed -nE 's/^test result: ok\. ([0-9]+) passed; ([0-9]+) failed; ([0-9]+) ignored;.*/\1 \2 \3/p' "$result" |
      tail -n 1
  )"
  if [[ -z "$counts" ]]; then
    printf 'ERROR: native contract produced no parseable libtest result: %s\n' "$test_filter" >&2
    exit 1
  fi
  read -r passed failed ignored <<< "$counts"
  if (( passed == 0 )); then
    printf 'ERROR: native contract filter matched no executed tests: %s\n' "$test_filter" >&2
    exit 1
  fi
  if (( failed != 0 || ignored != 0 )); then
    printf 'ERROR: native contract did not execute every selected test: %s (passed=%s failed=%s ignored=%s)\n' \
      "$test_filter" "$passed" "$failed" "$ignored" >&2
    exit 1
  fi
  printf 'PASS Native contract %s: %s executed, none ignored\n' "$test_filter" "$passed"
done

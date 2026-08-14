#!/usr/bin/env bash
set -uo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
PROFILE="core"
OUTPUT_DIR="${OSTUDY_DIAGNOSTIC_OUTPUT_DIR:-$ROOT_DIR/.diagnostics/agent}"

usage() {
  cat <<'EOF'
Usage: ./scripts/check-diagnostic.sh [--profile core|android|all] [--output-dir PATH]

Runs independent ostudy diagnostic gates without stopping after the first failure.
This is a development diagnostic sweep, not a replacement for the authoritative
check-static.sh, check-fast.sh, check.sh, or Android qualification workflows.
EOF
}

while (($# > 0)); do
  case "$1" in
    --profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:-}"
      shift 2
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

case "$PROFILE" in
  core|android|all) ;;
  *)
    printf 'Unsupported diagnostic profile: %s\n' "$PROFILE" >&2
    exit 2
    ;;
esac

export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
mkdir -p "$OUTPUT_DIR/logs"
RESULTS_FILE="$OUTPUT_DIR/results.tsv"
: > "$RESULTS_FILE"

# name | profile | status | exit_code | dependencies | log
printf 'name\tprofile\tstatus\texit_code\tdependencies\tlog\n' >> "$RESULTS_FILE"

declare -A GATE_STATUS=()
FAILED=0
BLOCKED=0

sanitize_name() {
  printf '%s' "$1" | tr -c 'A-Za-z0-9._-' '_'
}

record_gate() {
  local name="$1"
  local gate_profile="$2"
  local status="$3"
  local exit_code="$4"
  local dependencies="$5"
  local log="$6"
  GATE_STATUS["$name"]="$status"
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$name" "$gate_profile" "$status" "$exit_code" "$dependencies" "$log" \
    >> "$RESULTS_FILE"
}

should_block() {
  local dependencies="$1"
  local dependency
  [[ -z "$dependencies" ]] && return 1
  for dependency in $dependencies; do
    if [[ "${GATE_STATUS[$dependency]:-MISSING}" != "PASS" ]]; then
      return 0
    fi
  done
  return 1
}

run_gate() {
  local name="$1"
  local gate_profile="$2"
  local dependencies="$3"
  local timeout_value="$4"
  shift 4

  local safe_name log exit_code
  safe_name="$(sanitize_name "$name")"
  log="$OUTPUT_DIR/logs/${safe_name}.log"

  if should_block "$dependencies"; then
    printf 'BLOCKED %s (dependencies: %s)\n' "$name" "$dependencies" | tee "$log"
    record_gate "$name" "$gate_profile" "BLOCKED" "" "$dependencies" "logs/${safe_name}.log"
    BLOCKED=$((BLOCKED + 1))
    return 0
  fi

  printf '\n==> diagnostic: %s\n' "$name"
  printf 'Command:' > "$log"
  printf ' %q' "$@" >> "$log"
  printf '\n\n' >> "$log"

  set +e
  (
    cd "$ROOT_DIR"
    timeout --signal=TERM --kill-after=30s "$timeout_value" "$@"
  ) >> "$log" 2>&1
  exit_code=$?
  set -e

  if [[ "$exit_code" -eq 0 ]]; then
    printf 'PASS    %s\n' "$name"
    record_gate "$name" "$gate_profile" "PASS" "0" "$dependencies" "logs/${safe_name}.log"
  else
    printf 'FAIL    %s (exit %s)\n' "$name" "$exit_code"
    record_gate "$name" "$gate_profile" "FAIL" "$exit_code" "$dependencies" "logs/${safe_name}.log"
    FAILED=$((FAILED + 1))
  fi
}

run_core() {
  run_gate "doctor" "core" "" "10m" bash ./scripts/doctor.sh
  run_gate "project-manifests" "core" "" "5m" node ./scripts/verify-project-manifests.mjs
  run_gate "ipc-contract" "core" "" "5m" node ./scripts/verify-ipc-contract.mjs
  run_gate "repository-policy" "core" "" "5m" node ./scripts/verify-repository-policy.mjs
  run_gate "platform-boundaries" "core" "" "5m" node ./scripts/verify-platform-boundaries.mjs
  run_gate "external-file-streaming" "core" "" "5m" node ./scripts/verify-external-file-streaming.mjs
  run_gate "documentation" "core" "" "5m" node ./scripts/verify-documentation.mjs
  run_gate "test-policy" "core" "" "5m" node ./scripts/verify-test-policy.mjs
  run_gate "script-syntax" "core" "" "5m" bash ./scripts/verify-script-syntax.sh
  run_gate "script-tests" "core" "" "10m" python3 -m unittest discover -s tests/scripts -p 'test_*.py'
  run_gate "pnpm-install" "core" "" "20m" pnpm install --frozen-lockfile
  run_gate "js-versions" "core" "pnpm-install" "5m" pnpm run verify:versions
  run_gate "typescript" "core" "pnpm-install" "15m" pnpm exec tsc -b --pretty false
  run_gate "rustfmt" "core" "" "10m" cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
  run_gate "pdfium-runtime" "core" "" "20m" bash ./scripts/install-native-deps.sh --pdfium-only
  run_gate "cargo-metadata" "core" "" "10m" cargo metadata --manifest-path src-tauri/Cargo.toml --format-version 1 --locked
  run_gate "vitest" "core" "pnpm-install" "30m" pnpm test
  run_gate "cargo-check" "core" "cargo-metadata pdfium-runtime" "30m" cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features
  run_gate "clippy" "core" "cargo-metadata pdfium-runtime" "45m" cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features -- -D warnings
  run_gate "rust-tests" "core" "cargo-metadata pdfium-runtime" "60m" cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features
  run_gate "frontend-build" "core" "pnpm-install" "20m" pnpm run build
}

run_android() {
  run_gate "android-install-deps" "android" "" "30m" bash ./scripts/install-android-deps.sh --with-emulator
  run_gate "android-rust-targets" "android" "" "15m" rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
  run_gate "android-runner-doctor" "android" "android-install-deps" "15m" bash ./scripts/android-runner-doctor.sh
  run_gate "android-project" "android" "android-install-deps" "10m" node ./scripts/verify-android-project.mjs
  run_gate "android-streaming-contract" "android" "" "5m" node ./scripts/verify-external-file-streaming.mjs
  run_gate "android-gradle-dependencies" "android" "android-install-deps" "20m" bash -lc 'source ./scripts/android-env.sh; android_write_local_properties; cd src-tauri/gen/android; ./gradlew --no-daemon --parallel --max-workers "$OSTUDY_GRADLE_WORKERS" :tauri-plugin-ostudy-platform:dependencies --configuration debugCompileClasspath'
  run_gate "android-kotlin-compile" "android" "android-install-deps" "30m" bash -lc 'source ./scripts/android-env.sh; android_write_local_properties; cd src-tauri/gen/android; ./gradlew --no-daemon --parallel --max-workers "$OSTUDY_GRADLE_WORKERS" :tauri-plugin-ostudy-platform:compileDebugKotlin'
  run_gate "android-debug-build" "android" "android-install-deps android-rust-targets android-project" "75m" bash -lc 'source ./scripts/android-env.sh; android_write_local_properties; export VITE_OSTUDY_ANDROID_TEST_BRIDGE=1; bash ./scripts/android-build.sh --debug --apk --target x86_64 --smoke-bridge'
  run_gate "android-test-packages" "android" "android-debug-build" "30m" bash -lc 'source ./scripts/android-env.sh; android_write_local_properties; bash ./scripts/android-gradle-postbuild.sh test-packages'
  run_gate "android-emulator-smoke" "android" "android-debug-build android-runner-doctor" "90m" bash ./scripts/android-emulator-smoke.sh --artifacts "$OUTPUT_DIR/android-smoke"
}

case "$PROFILE" in
  core)
    run_core
    ;;
  android)
    run_android
    ;;
  all)
    run_core
    run_android
    ;;
esac

node "$SCRIPT_DIR/summarize-diagnostics.mjs" \
  --results "$RESULTS_FILE" \
  --output-dir "$OUTPUT_DIR" \
  --profile "$PROFILE"

printf '\nDiagnostic sweep complete: %s failed, %s blocked.\n' "$FAILED" "$BLOCKED"
printf 'Summary: %s\n' "$OUTPUT_DIR/summary.md"
printf 'Machine-readable results: %s\n' "$OUTPUT_DIR/diagnostics.json"

if ((FAILED > 0)); then
  exit 1
fi
exit 0

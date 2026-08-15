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
The same reusable gate implementations back authoritative fail-fast validation;
this runner only changes orchestration and failure collection.
EOF
}

while (($# > 0)); do
  case "$1" in
    --profile) PROFILE="${2:-}"; shift 2 ;;
    --output-dir) OUTPUT_DIR="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done
case "$PROFILE" in core|android|all) ;; *) printf 'Unsupported diagnostic profile: %s\n' "$PROFILE" >&2; exit 2 ;; esac

export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
mkdir -p "$OUTPUT_DIR/logs"
RESULTS_FILE="$OUTPUT_DIR/results.tsv"
printf 'name\tprofile\tstatus\texit_code\tduration_ms\tdependencies\tlog\n' > "$RESULTS_FILE"

declare -A GATE_STATUS=()
FAILED=0
BLOCKED=0

sanitize_name() { printf '%s' "$1" | tr -c 'A-Za-z0-9._-' '_'; }
record_gate() {
  local name="$1" gate_profile="$2" status="$3" exit_code="$4" duration_ms="$5" dependencies="$6" log="$7"
  GATE_STATUS["$name"]="$status"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$name" "$gate_profile" "$status" "$exit_code" "$duration_ms" "$dependencies" "$log" >> "$RESULTS_FILE"
}
should_block() {
  local dependencies="$1" dependency
  [[ -z "$dependencies" ]] && return 1
  for dependency in $dependencies; do
    [[ "${GATE_STATUS[$dependency]:-MISSING}" == "PASS" ]] || return 0
  done
  return 1
}
run_gate() {
  local name="$1" gate_profile="$2" dependencies="$3" timeout_value="$4"
  shift 4
  local safe_name log exit_code started finished duration_ms
  safe_name="$(sanitize_name "$name")"
  log="$OUTPUT_DIR/logs/${safe_name}.log"

  if should_block "$dependencies"; then
    printf 'BLOCKED %s (dependencies: %s)\n' "$name" "$dependencies" | tee "$log"
    record_gate "$name" "$gate_profile" "BLOCKED" "" "" "$dependencies" "logs/${safe_name}.log"
    BLOCKED=$((BLOCKED + 1))
    return 0
  fi

  printf '\n==> diagnostic: %s\n' "$name"
  printf 'Command:' > "$log"
  printf ' %q' "$@" >> "$log"
  printf '\n\n' >> "$log"
  started="$(date +%s%3N)"
  (
    cd "$ROOT_DIR"
    timeout --signal=TERM --kill-after=30s "$timeout_value" "$@"
  ) >> "$log" 2>&1
  exit_code=$?
  finished="$(date +%s%3N)"
  duration_ms=$((finished - started))

  if [[ "$exit_code" -eq 0 ]]; then
    printf 'PASS    %s (%sms)\n' "$name" "$duration_ms"
    record_gate "$name" "$gate_profile" "PASS" "0" "$duration_ms" "$dependencies" "logs/${safe_name}.log"
  else
    printf 'FAIL    %s (exit %s, %sms)\n' "$name" "$exit_code" "$duration_ms"
    record_gate "$name" "$gate_profile" "FAIL" "$exit_code" "$duration_ms" "$dependencies" "logs/${safe_name}.log"
    FAILED=$((FAILED + 1))
  fi
}

shared_gate() {
  local name="$1" dependencies="$2" timeout_value="$3" function_name="$4"
  run_gate "$name" "core" "$dependencies" "$timeout_value" \
    bash -c "source ./scripts/qualification-gates.sh; $function_name"
}
android_gate() {
  local name="$1" dependencies="$2" timeout_value="$3" function_name="$4"
  run_gate "$name" "android" "$dependencies" "$timeout_value" \
    bash -c "source ./scripts/android-qualification-gates.sh; $function_name"
}

run_core() {
  shared_gate doctor "" 10m ostudy_gate_doctor
  shared_gate project-manifests "" 5m ostudy_gate_project_manifests
  shared_gate ui-token-generation "" 5m ostudy_gate_ui_token_generation
  shared_gate ui-token-usage "" 5m ostudy_gate_ui_token_usage
  shared_gate ipc-contract "" 5m ostudy_gate_ipc_contract
  shared_gate repository-policy "" 5m ostudy_gate_repository_policy
  shared_gate platform-boundaries "" 5m ostudy_gate_platform_boundaries
  shared_gate external-file-streaming "" 5m ostudy_gate_external_file_streaming
  shared_gate documentation "" 5m ostudy_gate_documentation
  shared_gate lockfiles "" 2m ostudy_gate_lockfiles
  shared_gate test-policy "" 5m ostudy_gate_test_policy
  shared_gate script-syntax "" 5m ostudy_gate_script_syntax
  shared_gate script-tests "script-syntax" 15m ostudy_gate_script_tests
  shared_gate pnpm-install "lockfiles" 20m ostudy_gate_pnpm_install
  shared_gate js-versions "pnpm-install" 5m ostudy_gate_js_versions
  shared_gate typescript "pnpm-install" 15m ostudy_gate_typescript
  shared_gate rustfmt "" 10m ostudy_gate_rustfmt
  shared_gate pdfium-runtime "" 20m ostudy_gate_pdfium_runtime
  shared_gate cargo-metadata "lockfiles" 10m ostudy_gate_cargo_metadata_locked
  shared_gate vitest "pnpm-install" 35m ostudy_gate_vitest
  shared_gate cargo-check "cargo-metadata pdfium-runtime" 35m ostudy_gate_cargo_check
  shared_gate clippy "cargo-check" 45m ostudy_gate_clippy
  shared_gate rust-tests "cargo-check" 75m ostudy_gate_rust_tests
  shared_gate frontend-build "pnpm-install typescript" 25m ostudy_gate_frontend_build
}

run_android() {
  run_gate android-install-deps android "" 35m bash ./scripts/install-android-deps.sh --with-emulator
  run_gate android-rust-targets android "" 15m rustup target add \
    aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
  run_gate android-pnpm-install android "" 20m pnpm install --frozen-lockfile
  run_gate android-runner-doctor android "android-install-deps android-rust-targets" 15m \
    bash ./scripts/android-runner-doctor.sh
  android_gate android-project "android-install-deps" 10m ostudy_android_gate_project_contracts
  android_gate android-smoke-frontend "android-pnpm-install" 15m ostudy_android_gate_smoke_frontend

  # arm64 and x86_64 are independent once common prerequisites are ready; a
  # physical-device compile failure must not hide emulator-architecture defects.
  android_gate android-arm64-build \
    "android-install-deps android-rust-targets android-pnpm-install android-project" 90m \
    ostudy_android_gate_build_arm64_debug
  android_gate android-arm64-package "android-arm64-build" 10m ostudy_android_gate_verify_latest_app
  android_gate android-x86-smoke-build \
    "android-install-deps android-rust-targets android-pnpm-install android-project android-smoke-frontend" 90m \
    ostudy_android_gate_build_x86_smoke
  android_gate android-gradle-contracts "android-x86-smoke-build" 35m ostudy_android_gate_gradle_contracts
  android_gate android-gradle-lint-tests "android-x86-smoke-build" 60m ostudy_android_gate_test_packages
  android_gate android-test-packages "android-gradle-lint-tests" 10m ostudy_android_gate_verify_test_packages
  android_gate android-build-cleanliness "" 5m ostudy_android_gate_build_cleanliness

  local app_apk test_apk
  app_apk="$(bash -c 'source ./scripts/android-qualification-gates.sh; ostudy_android_find_app_apk' 2>/dev/null || true)"
  test_apk="$(bash -c 'source ./scripts/android-qualification-gates.sh; ostudy_android_find_test_apk' 2>/dev/null || true)"
  if [[ "${GATE_STATUS[android-x86-smoke-build]:-}" == PASS \
    && "${GATE_STATUS[android-test-packages]:-}" == PASS \
    && "${GATE_STATUS[android-runner-doctor]:-}" == PASS \
    && -f "$app_apk" && -f "$test_apk" ]]; then
    run_gate android-emulator-smoke android \
      "android-x86-smoke-build android-test-packages android-runner-doctor" 110m \
      bash ./scripts/android-emulator-smoke.sh \
        --app-apk "$app_apk" \
        --test-apk "$test_apk" \
        --artifacts "$OUTPUT_DIR/android-smoke"
  else
    run_gate android-emulator-smoke android \
      "android-x86-smoke-build android-test-packages android-runner-doctor" 110m true
  fi
}

case "$PROFILE" in
  core) run_core ;;
  android) run_android ;;
  all) run_core; run_android ;;
esac

if ! node "$SCRIPT_DIR/summarize-diagnostics.mjs" \
  --results "$RESULTS_FILE" \
  --output-dir "$OUTPUT_DIR" \
  --profile "$PROFILE"; then
  printf 'Diagnostic summarization failed. Raw gate results remain in %s.\n' "$RESULTS_FILE" >&2
  exit 2
fi

printf '\nDiagnostic sweep complete: %s failed, %s blocked.\n' "$FAILED" "$BLOCKED"
printf 'Summary: %s\n' "$OUTPUT_DIR/summary.md"
printf 'Machine-readable results: %s\n' "$OUTPUT_DIR/diagnostics.json"
((FAILED == 0))

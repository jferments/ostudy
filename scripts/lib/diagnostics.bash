#!/usr/bin/env bash
# Shared diagnostic-filter and failure-artifact handling for ostudy scripts.
# This file deliberately never enables shell xtrace: command tracing can expose
# credentials and user-controlled paths in CI and local terminal transcripts.

OSTUDY_DIAGNOSTIC_REMAINDER=()

ostudy_resolve_application_diagnostic_log() {
  if [[ -n "${OSTUDY_DIAGNOSTIC_LOG_PATH:-}" ]]; then
    printf '%s\n' "$OSTUDY_DIAGNOSTIC_LOG_PATH"
    return 0
  fi

  local app_data_dir
  if [[ -n "${OSTUDY_APP_DATA_DIR:-}" ]]; then
    app_data_dir="$OSTUDY_APP_DATA_DIR"
  else
    # Tauri app_data_dir resolves to data_dir/<bundle identifier>.
    app_data_dir="${XDG_DATA_HOME:-$HOME/.local/share}/io.github.jferments.ostudy"
  fi
  printf '%s\n' "$app_data_dir/diagnostics/ostudy.jsonl"
}

ostudy_diagnostic_artifact_root() {
  if [[ -n "${OSTUDY_DIAGNOSTIC_ARTIFACT_ROOT:-}" ]]; then
    printf '%s\n' "$OSTUDY_DIAGNOSTIC_ARTIFACT_ROOT"
    return 0
  fi
  printf '%s\n' "${OSTUDY_ROOT_DIR:-$(pwd)}/.artifacts/diagnostics"
}

ostudy_diagnostic_target() {
  case "$1" in
    app|startup) printf 'ostudy.app' ;;
    database|db|migration|migrations) printf 'ostudy.database' ;;
    ai|llm|openrouter) printf 'ostudy.ai' ;;
    qr|scanner) printf 'ostudy.qr' ;;
    sync|device-sync|device_sync) printf 'ostudy.device_sync' ;;
    document|documents|ocr) printf 'ostudy.documents' ;;
    frontend|webview|ui) printf 'ostudy.frontend' ;;
    filesystem|files) printf 'ostudy.filesystem' ;;
    diagnostics|logging) printf 'ostudy.diagnostics' ;;
    *)
      if [[ "$1" =~ ^[a-zA-Z0-9_.-]+$ ]]; then
        printf 'ostudy.%s' "${1,,}"
      else
        return 1
      fi
      ;;
  esac
}

ostudy_filter_for_targets() {
  local level="$1"
  local raw_targets="$2"
  local filter='warn'
  local target normalized
  IFS=',' read -r -a _ostudy_targets <<< "$raw_targets"
  for target in "${_ostudy_targets[@]}"; do
    target="${target//[[:space:]]/}"
    [[ -n "$target" ]] || continue
    if [[ "$target" == all ]]; then
      printf '%s' "$level"
      return 0
    fi
    normalized="$(ostudy_diagnostic_target "$target")" || {
      printf 'Invalid diagnostic subsystem: %s\n' "$target" >&2
      return 2
    }
    filter+=",$normalized=$level"
  done
  printf '%s' "$filter"
}

ostudy_parse_diagnostic_args() {
  OSTUDY_DIAGNOSTIC_REMAINDER=()
  local argument value explicit=0
  while (( $# > 0 )); do
    argument="$1"
    case "$argument" in
      --debug)
        export OSTUDY_LOG=debug
        export OSTUDY_DIAGNOSTICS_ACTIVE=1
        explicit=1
        ;;
      --debug=*)
        value="${argument#--debug=}"
        export OSTUDY_LOG="$(ostudy_filter_for_targets debug "$value")"
        export OSTUDY_DIAGNOSTICS_ACTIVE=1
        explicit=1
        ;;
      --trace)
        export OSTUDY_LOG=trace
        export OSTUDY_DIAGNOSTICS_ACTIVE=1
        explicit=1
        ;;
      --trace=*)
        value="${argument#--trace=}"
        export OSTUDY_LOG="$(ostudy_filter_for_targets trace "$value")"
        export OSTUDY_DIAGNOSTICS_ACTIVE=1
        explicit=1
        ;;
      --log-filter=*)
        value="${argument#--log-filter=}"
        [[ -n "$value" && "$value" != *$'\n'* && "$value" != *$'\r'* ]] || {
          printf 'Diagnostic filters cannot be empty or contain newlines.\n' >&2
          return 2
        }
        export OSTUDY_LOG="$value"
        export OSTUDY_DIAGNOSTICS_ACTIVE=1
        explicit=1
        ;;
      --keep-artifacts)
        export OSTUDY_KEEP_FAILURE_ARTIFACTS=1
        ;;
      --)
        shift
        OSTUDY_DIAGNOSTIC_REMAINDER+=("$@")
        break
        ;;
      *) OSTUDY_DIAGNOSTIC_REMAINDER+=("$argument") ;;
    esac
    shift
  done

  if [[ -n "${OSTUDY_LOG:-}" ]]; then
    export OSTUDY_DIAGNOSTICS_ACTIVE=1
  elif [[ "${CI:-false}" == true ]]; then
    # CI keeps a low-noise internal timeline on the first run. This is much
    # cheaper than rerunning an expensive gate after the original state is gone.
    export OSTUDY_LOG="${OSTUDY_CI_LOG_FILTER:-warn,ostudy=info}"
    export OSTUDY_DIAGNOSTICS_ACTIVE=1
    export OSTUDY_DIAGNOSTICS_AUTOMATIC=1
  fi

  if [[ "${CI:-false}" == true ]]; then
    export OSTUDY_KEEP_FAILURE_ARTIFACTS=1
  fi

  if [[ "${OSTUDY_KEEP_FAILURE_ARTIFACTS:-0}" == 1 && -z "${OSTUDY_DIAGNOSTIC_CAPTURE_TOKEN:-}" ]]; then
    # Nested qualification scripts inherit one token. The first failing boundary
    # claims it atomically, preventing outer wrapper traps from replacing a
    # precise failure with duplicate "initialization" captures.
    export OSTUDY_DIAGNOSTIC_CAPTURE_TOKEN="${BASHPID:-$$}-${RANDOM:-0}-$(date -u +%s%N 2>/dev/null || date -u +%s)"
  fi

  if [[ "${OSTUDY_DIAGNOSTICS_ACTIVE:-0}" == 1 ]]; then
    export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
    if [[ "$explicit" == 1 || "${OSTUDY_DIAGNOSTICS_AUTOMATIC:-0}" != 1 ]]; then
      export RUST_TEST_NOCAPTURE=1
      export OSTUDY_FORCE_CHECK=1
    fi
    printf 'Diagnostic filter: %s\n' "$OSTUDY_LOG"
  fi
}

ostudy_diagnostic_usage() {
  cat <<'EOF'
Diagnostic options:
  --debug                    Enable application-wide debug events
  --debug=db,ai,qr           Enable debug events for selected subsystems
  --trace[=subsystems]       Enable trace events globally or selectively
  --log-filter=<filter>      Pass an exact OSTUDY_LOG target filter
  --keep-artifacts           Retain bounded failure diagnostics
EOF
}

ostudy_copy_bounded_log() {
  local source="$1"
  local destination="$2"
  [[ -f "$source" ]] || return 0
  tail -n "${OSTUDY_DIAGNOSTIC_TAIL_LINES:-2000}" -- "$source" > "$destination"
}

ostudy_recommended_filter_for_step() {
  local normalized="${1,,}"
  case "$normalized" in
    *database*|*migration*|*schema*|*sqlite*)
      printf 'warn,ostudy.database=debug'
      ;;
    *openrouter*|*provider*|*document*|*pdf*|*ocr*|*ai*)
      printf 'warn,ostudy.ai=debug,ostudy.documents=debug'
      ;;
    *qr*|*scanner*)
      printf 'warn,ostudy.app=info,ostudy.qr=debug,ostudy.device_sync=debug'
      ;;
    *android*)
      printf 'warn,ostudy.app=debug,ostudy.qr=debug,ostudy.filesystem=debug'
      ;;
    *webdriver*|*frontend*|*react*|*tauri*)
      printf 'warn,ostudy.app=debug,ostudy.frontend=debug'
      ;;
    *external\ file*|*filesystem*|*asset*|*import*|*export*)
      printf 'warn,ostudy.filesystem=debug'
      ;;
    *)
      printf 'warn,ostudy=debug'
      ;;
  esac
}

ostudy_replay_command_for_step() {
  local step="$1"
  case "$step" in
    "React tests")
      printf 'pnpm test'
      ;;
    "Rust compile check")
      printf 'cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features'
      ;;
    "Rust warnings-as-errors lint gate"|"Rust linting")
      printf 'cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features -- -D warnings'
      ;;
    "External file transfer and recovery contracts")
      printf 'cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib external_file_ -- --nocapture'
      ;;
    "Document domain and persistence contracts")
      printf 'cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib document_ -- --nocapture'
      ;;
    "PDF extraction, rendering, and OCR contracts")
      printf 'cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib pdf_ -- --nocapture'
      ;;
    "Latest-schema migration contract")
      printf 'cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib database::tests::public_baseline_is_idempotent_and_matches_manifest -- --exact --nocapture'
      ;;
    "Public migration immutability contract")
      printf 'cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib database::tests::public_migration_checksum_and_future_version_are_enforced -- --exact --nocapture'
      ;;
    "Frontend production build")
      printf 'pnpm run build'
      ;;
    "Rust unit, integration, and migration-fixture tests")
      printf 'cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features -- --nocapture'
      ;;
    "Standalone Tauri build")
      printf 'pnpm tauri build --debug --no-bundle --ci -- --locked'
      ;;
    "Native Tauri/WebDriver smoke test"|Native\ *WebDriver\ smoke*)
      printf './scripts/smoke-native.sh'
      ;;
    "Android Emulator smoke qualification"|Android\ *smoke*)
      printf './scripts/android-emulator-smoke.sh'
      ;;
    "Environment and repository contracts"|"Behavior-test policy and repository script validation"|"Frozen JavaScript installation and dependency verification"|"Frontend type checking"|"Rust formatting")
      printf './scripts/check-static.sh'
      ;;
    *)
      printf '%s' "${OSTUDY_QUALIFICATION_COMMAND:-./scripts/check.sh}"
      ;;
  esac
}

ostudy_step_slug() {
  local slug
  slug="$(printf '%s' "$1" | tr '[:upper:] /' '[:lower:]--' | tr -cd '[:alnum:]_.-')"
  printf '%s' "${slug:-failure}"
}

ostudy_capture_artifacts() {
  local kind="$1"
  local step="$2"
  local status="$3"
  local command_log="${4:-}"
  local application_override="${5:-}"
  [[ "${OSTUDY_KEEP_FAILURE_ARTIFACTS:-0}" == 1 || "${CI:-false}" == true ]] || return 0

  local root="${OSTUDY_ROOT_DIR:-$(pwd)}"
  local timestamp artifact_root artifact_dir state_root application_log application_previous
  local replay_filter replay_command commit='unknown'
  timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
  artifact_root="$(ostudy_diagnostic_artifact_root)"
  mkdir -p "$artifact_root"
  if [[ "$kind" == failure && -n "${OSTUDY_DIAGNOSTIC_CAPTURE_TOKEN:-}" ]]; then
    local capture_token failure_claim
    capture_token="${OSTUDY_DIAGNOSTIC_CAPTURE_TOKEN//[^a-zA-Z0-9_.-]/_}"
    failure_claim="$artifact_root/.failure-claim-$capture_token"
    mkdir "$failure_claim" 2>/dev/null || return 0
  fi
  artifact_dir="$(mktemp -d "$artifact_root/${timestamp}-$(ostudy_step_slug "$step").XXXXXX")"
  state_root="${XDG_STATE_HOME:-$HOME/.local/state}/ostudy"
  application_log="${application_override:-$(ostudy_resolve_application_diagnostic_log)}"
  application_previous="$(dirname -- "$application_log")/ostudy.previous.jsonl"
  replay_filter="$(ostudy_recommended_filter_for_step "$step")"
  replay_command="$(ostudy_replay_command_for_step "$step")"
  if command -v git >/dev/null 2>&1; then
    commit="$(git -C "$root" rev-parse HEAD 2>/dev/null || printf unknown)"
  fi

  {
    printf 'kind=%s\n' "$kind"
    printf 'step=%s\n' "$step"
    printf 'status=%s\n' "$status"
    printf 'utc=%s\n' "$timestamp"
    printf 'sourceScript=%s\n' "$(basename -- "$0")"
    printf 'filter=%s\n' "${OSTUDY_LOG:-info}"
    printf 'replayFilter=%s\n' "$replay_filter"
    printf 'replayCommand=%s\n' "$replay_command"
    printf 'commit=%s\n' "$commit"
  } > "$artifact_dir/context.txt"

  {
    printf '#!/usr/bin/env bash\n'
    printf 'set -Eeuo pipefail\n'
    printf 'cd %q\n' "$root"
    printf 'export OSTUDY_FORCE_CHECK=1\n'
    printf 'export OSTUDY_KEEP_FAILURE_ARTIFACTS=1\n'
    printf 'export OSTUDY_LOG=%q\n' "$replay_filter"
    printf '%s\n' "$replay_command"
  } > "$artifact_dir/reproduce.sh"
  chmod 700 "$artifact_dir/reproduce.sh"

  ostudy_copy_bounded_log "$state_root/ostudy.jsonl" "$artifact_dir/launcher.jsonl"
  ostudy_copy_bounded_log "$state_root/ostudy.jsonl.1" "$artifact_dir/launcher.previous.jsonl"
  ostudy_copy_bounded_log "$application_log" "$artifact_dir/application.jsonl"
  ostudy_copy_bounded_log "$application_previous" "$artifact_dir/application.previous.jsonl"
  if [[ -n "$command_log" ]]; then
    ostudy_copy_bounded_log "$command_log" "$artifact_dir/command.log"
  fi

  printf '%s\n' "$artifact_dir" >> "$artifact_root/index.txt"
  printf 'Diagnostic artifacts: %s\n' "$artifact_dir" >&2
  printf 'Focused replay: %s\n' "$artifact_dir/reproduce.sh" >&2
}

ostudy_capture_failure_artifacts() {
  ( set +e; ostudy_capture_artifacts failure "$@" ) || true
}

ostudy_capture_transient_artifacts() {
  ( set +e; ostudy_capture_artifacts transient "$@" ) || true
}

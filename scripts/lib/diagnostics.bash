#!/usr/bin/env bash
# Shared diagnostic-filter and failure-artifact handling for ostudy scripts.
# This file deliberately never enables shell xtrace: command tracing can expose
# credentials and user-controlled paths in CI and local terminal transcripts.

OSTUDY_DIAGNOSTIC_REMAINDER=()

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
  local argument value
  while (( $# > 0 )); do
    argument="$1"
    case "$argument" in
      --debug)
        export OSTUDY_LOG=debug
        export OSTUDY_DIAGNOSTICS_ACTIVE=1
        ;;
      --debug=*)
        value="${argument#--debug=}"
        export OSTUDY_LOG="$(ostudy_filter_for_targets debug "$value")"
        export OSTUDY_DIAGNOSTICS_ACTIVE=1
        ;;
      --trace)
        export OSTUDY_LOG=trace
        export OSTUDY_DIAGNOSTICS_ACTIVE=1
        ;;
      --trace=*)
        value="${argument#--trace=}"
        export OSTUDY_LOG="$(ostudy_filter_for_targets trace "$value")"
        export OSTUDY_DIAGNOSTICS_ACTIVE=1
        ;;
      --log-filter=*)
        value="${argument#--log-filter=}"
        [[ -n "$value" && "$value" != *$'\n'* && "$value" != *$'\r'* ]] || {
          printf 'Diagnostic filters cannot be empty or contain newlines.\n' >&2
          return 2
        }
        export OSTUDY_LOG="$value"
        export OSTUDY_DIAGNOSTICS_ACTIVE=1
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

  if [[ "${OSTUDY_DIAGNOSTICS_ACTIVE:-0}" == 1 ]]; then
    export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
    export RUST_TEST_NOCAPTURE=1
    export OSTUDY_FORCE_CHECK=1
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
  tail -n 2000 -- "$source" > "$destination"
}

ostudy_capture_failure_artifacts() {
  local step="$1"
  local status="$2"
  [[ "${OSTUDY_KEEP_FAILURE_ARTIFACTS:-0}" == 1 || "${CI:-false}" == true ]] || return 0

  local root="${OSTUDY_ROOT_DIR:-$(pwd)}"
  local timestamp artifact_dir state_root data_root
  timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
  artifact_dir="$root/.artifacts/diagnostics/$timestamp"
  state_root="${XDG_STATE_HOME:-$HOME/.local/state}/ostudy"
  data_root="${XDG_DATA_HOME:-$HOME/.local/share}/ostudy/diagnostics"
  mkdir -p "$artifact_dir"
  {
    printf 'step=%s\n' "$step"
    printf 'status=%s\n' "$status"
    printf 'utc=%s\n' "$timestamp"
    printf 'filter=%s\n' "${OSTUDY_LOG:-info}"
    if command -v git >/dev/null 2>&1; then
      printf 'commit=%s\n' "$(git -C "$root" rev-parse HEAD 2>/dev/null || printf unknown)"
    fi
  } > "$artifact_dir/context.txt"
  ostudy_copy_bounded_log "$state_root/ostudy.jsonl" "$artifact_dir/launcher.jsonl"
  ostudy_copy_bounded_log "$state_root/ostudy.jsonl.1" "$artifact_dir/launcher.previous.jsonl"
  ostudy_copy_bounded_log "$data_root/ostudy.jsonl" "$artifact_dir/application.jsonl"
  ostudy_copy_bounded_log "$data_root/ostudy.previous.jsonl" "$artifact_dir/application.previous.jsonl"
  printf 'Diagnostic artifacts: %s\n' "$artifact_dir" >&2
}

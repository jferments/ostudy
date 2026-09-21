#!/usr/bin/env bash

# Lightweight wall-clock phase timing for maintained qualification wrappers.
# Uses Bash's monotonic SECONDS counter so timing never changes qualification
# behavior or requires an external timing dependency.

OSTUDY_TIMING_SCOPE=""
OSTUDY_TIMING_TOTAL_STARTED=0
OSTUDY_TIMING_TOTAL_ACTIVE=0
OSTUDY_TIMING_STEP=""
OSTUDY_TIMING_STEP_STARTED=0
OSTUDY_TIMING_STEP_ACTIVE=0
OSTUDY_TIMING_SUMMARY_STARTED=0

ostudy_timing_summary_start() {
  if [[ "${OSTUDY_TIMING_SUMMARY_STARTED:-0}" == 1 || -z "${GITHUB_STEP_SUMMARY:-}" ]]; then
    return 0
  fi
  {
    printf '\n### %s qualification timings\n\n' "$OSTUDY_TIMING_SCOPE"
    printf '| Phase | Status | Seconds |\n'
    printf '| --- | --- | ---: |\n'
  } >> "$GITHUB_STEP_SUMMARY"
  OSTUDY_TIMING_SUMMARY_STARTED=1
}

ostudy_timing_emit() {
  local phase="$1" status="$2" elapsed="$3"
  printf 'TIMING scope=%s status=%s elapsed_seconds=%s phase=%s\n' \
    "$OSTUDY_TIMING_SCOPE" "$status" "$elapsed" "$phase"
  ostudy_timing_summary_start
  if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    phase="${phase//|/\\|}"
    printf '| %s | %s | %s |\n' "$phase" "$status" "$elapsed" >> "$GITHUB_STEP_SUMMARY"
  fi
}

ostudy_timing_init() {
  OSTUDY_TIMING_SCOPE="$1"
  OSTUDY_TIMING_TOTAL_STARTED=$SECONDS
  OSTUDY_TIMING_TOTAL_ACTIVE=1
  OSTUDY_TIMING_STEP=""
  OSTUDY_TIMING_STEP_ACTIVE=0
  OSTUDY_TIMING_SUMMARY_STARTED=0
}

ostudy_timing_start_step() {
  OSTUDY_TIMING_STEP="$1"
  OSTUDY_TIMING_STEP_STARTED=$SECONDS
  OSTUDY_TIMING_STEP_ACTIVE=1
}

ostudy_timing_finish_step() {
  local status="${1:-passed}"
  if [[ "${OSTUDY_TIMING_STEP_ACTIVE:-0}" != 1 ]]; then
    return 0
  fi
  local elapsed=$((SECONDS - OSTUDY_TIMING_STEP_STARTED))
  ostudy_timing_emit "$OSTUDY_TIMING_STEP" "$status" "$elapsed"
  OSTUDY_TIMING_STEP_ACTIVE=0
}

ostudy_timing_finish_total() {
  local status="${1:-passed}"
  if [[ "${OSTUDY_TIMING_TOTAL_ACTIVE:-0}" != 1 ]]; then
    return 0
  fi
  local elapsed=$((SECONDS - OSTUDY_TIMING_TOTAL_STARTED))
  ostudy_timing_emit "TOTAL" "$status" "$elapsed"
  OSTUDY_TIMING_TOTAL_ACTIVE=0
}

#!/usr/bin/env bash

# Stable, machine-readable stage events for the optional developer console.
# Ordinary shell output remains authoritative and unchanged when the console is off.
ostudy_console_stage() {
  local scope="$1"
  shift
  [[ "${OSTUDY_CONSOLE_EVENTS:-0}" == "1" ]] || return 0
  local message="$*"
  message="${message//$'\t'/ }"
  message="${message//$'\n'/ }"
  printf '@@OSTUDY:TUI@@\tstage\t%s\t%s\n' "$scope" "$message"
}

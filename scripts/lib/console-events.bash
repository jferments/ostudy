#!/usr/bin/env bash

# Stable, machine-readable control events for the optional developer console.
# Ordinary shell output remains authoritative and unchanged when the console is off.
ostudy_console_sanitize_message() {
  local message="$*"
  message="${message//$'\t'/ }"
  message="${message//$'\n'/ }"
  printf '%s' "$message"
}

ostudy_console_stage() {
  local scope="$1"
  shift
  [[ "${OSTUDY_CONSOLE_EVENTS:-0}" == "1" ]] || return 0
  local message
  message="$(ostudy_console_sanitize_message "$@")"
  printf '@@OSTUDY:TUI@@\tstage\t%s\t%s\n' "$scope" "$message"
}

# Temporarily hand exclusive terminal ownership to a command that may prompt,
# open a curses/dialog UI, or otherwise require a real controlling terminal.
#
# The developer console supplies OSTUDY_CONSOLE_TAKEOVER_ACK. The shell emits a
# takeover request and waits for the Ratatui parent to fully restore the normal
# terminal before opening /dev/tty. When the command exits, the shell emits an
# end event; the parent then re-enters Ratatui. Without the developer console,
# the command executes normally with the caller's existing stdio.
ostudy_console_takeover() {
  local description="$1"
  shift
  if (($# == 0)); then
    printf 'ERROR: ostudy_console_takeover requires a command.\n' >&2
    return 2
  fi

  if [[ "${OSTUDY_CONSOLE_EVENTS:-0}" != "1" || -z "${OSTUDY_CONSOLE_TAKEOVER_ACK:-}" ]]; then
    "$@"
    return $?
  fi

  local terminal_device="${OSTUDY_CONSOLE_TTY:-/dev/tty}"
  if [[ ! -r "$terminal_device" || ! -w "$terminal_device" ]]; then
    "$@"
    return $?
  fi

  local message ack_path attempt status
  message="$(ostudy_console_sanitize_message "$description")"
  ack_path="$OSTUDY_CONSOLE_TAKEOVER_ACK"
  rm -f -- "$ack_path"
  printf '@@OSTUDY:TUI@@\ttakeover\tstart\t%s\n' "$message"

  # The TUI polls at 100 ms. Wait up to five seconds so the interactive command
  # can never race the parent for raw-mode keyboard input.
  for ((attempt = 0; attempt < 200; attempt += 1)); do
    [[ -f "$ack_path" ]] && break
    sleep 0.025
  done
  if [[ ! -f "$ack_path" ]]; then
    printf '@@OSTUDY:TUI@@\ttakeover\tend\t125\t%s\n' "$message"
    printf 'ERROR: developer console did not release terminal control for: %s\n' "$message" >&2
    return 125
  fi
  rm -f -- "$ack_path"

  status=0
  if "$@" <"$terminal_device" >"$terminal_device" 2>&1; then
    status=0
  else
    status=$?
  fi
  printf '@@OSTUDY:TUI@@\ttakeover\tend\t%s\t%s\n' "$status" "$message"
  return "$status"
}

# Require valid sudo credentials without ever allowing sudo itself to prompt
# while Ratatui owns the terminal. A cached credential stays inside the TUI;
# an expired/missing credential uses the takeover protocol above.
ostudy_console_require_sudo() {
  if sudo -n true 2>/dev/null; then
    return 0
  fi
  ostudy_console_takeover "sudo authentication" sudo -v
}

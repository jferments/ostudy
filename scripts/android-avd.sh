#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

COMMAND="${1:-}"
[[ $# -gt 0 ]] && shift
AVD_NAME="$ANDROID_PHONE_AVD"
SYSTEM_IMAGE="$ANDROID_PHONE_SYSTEM_IMAGE"
HEADLESS=0
WIPE=0
TIMEOUT_SECONDS=240

usage() {
  cat <<'USAGE'
Usage: ./scripts/android-avd.sh <create|start|wait|stop|status> [options]

Options:
  --name <avd>           Use another AVD name.
  --system-image <pkg>   Use another installed SDK system-image package.
  --headless             Start without an emulator window (CI mode).
  --wipe-data            Start from a clean data image.
  --timeout <sec>        Boot timeout, default 240 seconds.
USAGE
}

while (($#)); do
  case "$1" in
    --name) shift; AVD_NAME="${1:?--name requires a value}" ;;
    --system-image) shift; SYSTEM_IMAGE="${1:?--system-image requires a value}" ;;
    --headless) HEADLESS=1 ;;
    --wipe-data) WIPE=1 ;;
    --timeout) shift; TIMEOUT_SECONDS="${1:?--timeout requires seconds}" ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown AVD option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

adb="$ANDROID_HOME/platform-tools/adb"
emulator="$ANDROID_HOME/emulator/emulator"
avdmanager="$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager"
safe_avd_name="$(printf '%s' "$AVD_NAME" | tr -c 'A-Za-z0-9._-' '_')"
PID_FILE="$ROOT_DIR/.android-emulator-${safe_avd_name}.pid"
LOG_FILE="$ROOT_DIR/android-emulator-${safe_avd_name}.log"

connected_emulator_name() {
  local serial="$1"
  "$adb" -s "$serial" emu avd name 2>/dev/null | tr -d '\r' | awk 'NF && $0 != "OK" { print; exit }'
}

connected_emulator_serial_for_avd() {
  local serial current_name
  while read -r serial; do
    [[ -n "$serial" ]] || continue
    current_name="$(connected_emulator_name "$serial")"
    if [[ "$current_name" == "$AVD_NAME" ]]; then
      printf '%s\n' "$serial"
      return 0
    fi
  done < <("$adb" devices | awk 'NR > 1 && $1 ~ /^emulator-/ && $2 == "device" { print $1 }')
  return 1
}

wait_for_avd_disconnect() {
  local deadline
  deadline=$((SECONDS + 30))
  while ((SECONDS < deadline)); do
    if [[ -z "$(connected_emulator_serial_for_avd || true)" ]]; then
      return 0
    fi
    sleep 1
  done
  printf 'Android emulator %s did not disconnect within 30 seconds.\n' "$AVD_NAME" >&2
  return 1
}

create_avd() {
  bash "$SCRIPT_DIR/android-doctor.sh" --emulator
  if "$emulator" -list-avds | grep -Fxq "$AVD_NAME"; then
    printf 'Android AVD %s already exists.\n' "$AVD_NAME"
    return
  fi
  printf 'Creating Android AVD %s from %s...\n' "$AVD_NAME" "$SYSTEM_IMAGE"
  printf 'no\n' | "$avdmanager" create avd \
    --force \
    --name "$AVD_NAME" \
    --package "$SYSTEM_IMAGE"
  config="$HOME/.android/avd/${AVD_NAME}.avd/config.ini"
  {
    printf '\nhw.keyboard=yes\n'
    printf 'hw.ramSize=4096\n'
    printf 'disk.dataPartition.size=8G\n'
    printf 'showDeviceFrame=no\n'
  } >> "$config"
}

start_avd() {
  bash "$SCRIPT_DIR/android-doctor.sh" --emulator
  if ! "$emulator" -list-avds | grep -Fxq "$AVD_NAME"; then
    create_avd
  fi

  local serial
  serial="$(connected_emulator_serial_for_avd || true)"
  if [[ -n "$serial" ]]; then
    if ((WIPE == 0)); then
      printf 'Android emulator %s is already connected as %s.\n' "$AVD_NAME" "$serial"
      wait_for_boot
      return
    fi
    printf 'Restarting Android emulator %s with a clean data image.\n' "$AVD_NAME"
    stop_avd >/dev/null
    wait_for_avd_disconnect
  fi

  args=(
    -avd "$AVD_NAME"
    -no-snapshot-save
    -no-boot-anim
    -gpu swiftshader
    -no-audio
  )
  ((HEADLESS == 1)) && args+=(-no-window)
  ((WIPE == 1)) && args+=(-wipe-data)
  printf 'Starting Android Emulator %s (%s)...\n' "$AVD_NAME" "$SYSTEM_IMAGE"
  "$emulator" "${args[@]}" > "$LOG_FILE" 2>&1 &
  printf '%s\n' "$!" > "$PID_FILE"
  wait_for_boot
}

wait_for_boot() {
  "$adb" start-server >/dev/null
  local deadline serial current_name
  deadline=$((SECONDS + TIMEOUT_SECONDS))
  while ((SECONDS < deadline)); do
    serial="$(connected_emulator_serial_for_avd || true)"
    if [[ -n "$serial" ]] && [[ "$($adb -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == 1 ]]; then
      current_name="$(connected_emulator_name "$serial")"
      "$adb" -s "$serial" shell input keyevent 82 >/dev/null 2>&1 || true
      "$adb" -s "$serial" shell settings put global window_animation_scale 0 >/dev/null
      "$adb" -s "$serial" shell settings put global transition_animation_scale 0 >/dev/null
      "$adb" -s "$serial" shell settings put global animator_duration_scale 0 >/dev/null
      printf 'Android Emulator ready: %s AVD=%s (API %s, page size %s).\n' \
        "$serial" \
        "${current_name:-unknown}" \
        "$($adb -s "$serial" shell getprop ro.build.version.sdk | tr -d '\r')" \
        "$($adb -s "$serial" shell getconf PAGE_SIZE | tr -d '\r')"
      return
    fi
    sleep 2
  done
  printf 'Android Emulator %s did not finish booting within %s seconds.\n' "$AVD_NAME" "$TIMEOUT_SECONDS" >&2
  tail -n 100 "$LOG_FILE" >&2 || true
  exit 1
}

stop_avd() {
  local serial stopped=0 pid
  while read -r serial; do
    [[ -n "$serial" ]] || continue
    if [[ "$(connected_emulator_name "$serial")" == "$AVD_NAME" ]]; then
      "$adb" -s "$serial" emu kill >/dev/null 2>&1 || true
      stopped=1
    fi
  done < <("$adb" devices | awk 'NR > 1 && $1 ~ /^emulator-/ { print $1 }')

  if [[ -f "$PID_FILE" ]]; then
    pid="$(cat "$PID_FILE")"
    kill "$pid" >/dev/null 2>&1 || true
    rm -f "$PID_FILE"
    stopped=1
  fi
  if ((stopped == 1)); then
    printf 'Stopped Android emulator %s.\n' "$AVD_NAME"
  else
    printf 'Android emulator %s was not running.\n' "$AVD_NAME"
  fi
}

status_avd() {
  "$adb" devices -l
  "$emulator" -list-avds
}

case "$COMMAND" in
  create) create_avd ;;
  start) start_avd ;;
  wait) wait_for_boot ;;
  stop) stop_avd ;;
  status) status_avd ;;
  *) usage >&2; exit 2 ;;
esac

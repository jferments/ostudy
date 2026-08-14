#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

COMMAND="${1:-}"
[[ $# -gt 0 ]] && shift
AVD_NAME="$ANDROID_PHONE_AVD"
HEADLESS=0
WIPE=0
TIMEOUT_SECONDS=240
PID_FILE="$ROOT_DIR/.android-emulator.pid"

usage() {
  cat <<'USAGE'
Usage: ./scripts/android-avd.sh <create|start|wait|stop|status> [options]

Options:
  --name <avd>       Use another AVD name.
  --headless         Start without an emulator window (CI mode).
  --wipe-data        Start from a clean data image.
  --timeout <sec>    Boot timeout, default 240 seconds.
USAGE
}

while (($#)); do
  case "$1" in
    --name) shift; AVD_NAME="${1:?--name requires a value}" ;;
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

create_avd() {
  bash "$SCRIPT_DIR/android-doctor.sh" --emulator
  if "$emulator" -list-avds | grep -Fxq "$AVD_NAME"; then
    printf 'Android AVD %s already exists.\n' "$AVD_NAME"
    return
  fi
  printf 'Creating Android AVD %s from %s...\n' "$AVD_NAME" "$ANDROID_PHONE_SYSTEM_IMAGE"
  printf 'no\n' | "$avdmanager" create avd \
    --force \
    --name "$AVD_NAME" \
    --package "$ANDROID_PHONE_SYSTEM_IMAGE"
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
  if "$adb" devices | awk 'NR > 1 && $2 == "device" { print $1 }' | grep -q '^emulator-'; then
    printf 'An Android emulator is already connected.\n'
    wait_for_boot
    return
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
  printf 'Starting Android Emulator %s...\n' "$AVD_NAME"
  "$emulator" "${args[@]}" > "$ROOT_DIR/android-emulator.log" 2>&1 &
  printf '%s\n' "$!" > "$PID_FILE"
  wait_for_boot
}

wait_for_boot() {
  "$adb" start-server >/dev/null
  deadline=$((SECONDS + TIMEOUT_SECONDS))
  serial=""
  while ((SECONDS < deadline)); do
    serial="$($adb devices | awk 'NR > 1 && $2 == "device" && $1 ~ /^emulator-/ { print $1; exit }')"
    if [[ -n "$serial" ]] && [[ "$($adb -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == 1 ]]; then
      "$adb" -s "$serial" shell input keyevent 82 >/dev/null 2>&1 || true
      "$adb" -s "$serial" shell settings put global window_animation_scale 0 >/dev/null
      "$adb" -s "$serial" shell settings put global transition_animation_scale 0 >/dev/null
      "$adb" -s "$serial" shell settings put global animator_duration_scale 0 >/dev/null
      printf 'Android Emulator ready: %s (API %s, page size %s).\n' \
        "$serial" \
        "$($adb -s "$serial" shell getprop ro.build.version.sdk | tr -d '\r')" \
        "$($adb -s "$serial" shell getconf PAGE_SIZE | tr -d '\r')"
      return
    fi
    sleep 2
  done
  printf 'Android Emulator did not finish booting within %s seconds.\n' "$TIMEOUT_SECONDS" >&2
  tail -n 100 "$ROOT_DIR/android-emulator.log" >&2 || true
  exit 1
}

stop_avd() {
  mapfile -t serials < <("$adb" devices | awk 'NR > 1 && $1 ~ /^emulator-/ { print $1 }')
  for serial in "${serials[@]}"; do
    "$adb" -s "$serial" emu kill >/dev/null 2>&1 || true
  done
  if [[ -f "$PID_FILE" ]]; then
    pid="$(cat "$PID_FILE")"
    kill "$pid" >/dev/null 2>&1 || true
    rm -f "$PID_FILE"
  fi
  printf 'Stopped Android emulator processes.\n'
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

#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
PACKAGE_NAME="io.github.jferments.ostudy.dev"
SERIAL=""
CAPTURE_SECONDS=0
OUTPUT_ROOT="${OSTUDY_USB_DIAGNOSTICS_DIR:-$ROOT_DIR/.artifacts/usb-sync-diagnostics}"

usage() {
  cat <<'USAGE'
Usage: ./scripts/usb-sync-diagnostics.sh [options]

Capture a physical Linux <-> Android USB Sync attempt while Android USB
Debugging is enabled. The command correlates Linux enumeration/udev events,
Android USB state and filtered logcat, the installed app's accessory intent
registration, and the desktop ostudy process output.

The script launches the already-built desktop application itself. Close any
existing desktop ostudy window before starting it. It does not rebuild either
platform and does not modify phone data.

Options:
  --serial <adb-serial>  Select one authorized Android device.
  --seconds <n>          Capture for n seconds instead of waiting for Enter.
  --output-root <path>   Parent directory for diagnostic captures.
  -h, --help             Show this help.

Recommended first diagnostic:
  1. Enable USB debugging and authorize this Linux computer.
  2. Unlock the phone and connect one data-capable USB cable.
  3. Run this command.
  4. When the desktop app appears, click USB Sync exactly once.
  5. Accept any Android system USB/accessory prompt that appears.
  6. After success/failure is visible, return here and press Enter.
USAGE
}

while (($#)); do
  case "$1" in
    --serial)
      shift
      SERIAL="${1:?--serial requires an adb device serial}"
      ;;
    --seconds)
      shift
      CAPTURE_SECONDS="${1:?--seconds requires a positive integer}"
      [[ "$CAPTURE_SECONDS" =~ ^[1-9][0-9]*$ ]] || {
        printf '%s\n' '--seconds requires a positive integer.' >&2
        exit 2
      }
      ;;
    --output-root)
      shift
      OUTPUT_ROOT="${1:?--output-root requires a path}"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'Unknown USB diagnostic option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

cd "$ROOT_DIR"

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    printf 'Required diagnostic command is missing: %s\n' "$1" >&2
    exit 1
  }
}

for command in adb awk date grep sed sort tar timeout; do
  require_command "$command"
done

[[ "$(uname -s)" == "Linux" ]] || {
  printf 'USB Sync host diagnostics currently require Linux.\n' >&2
  exit 1
}

adb_args=()
if [[ -n "$SERIAL" ]]; then
  adb_args=(-s "$SERIAL")
fi
mapfile -t devices < <(adb devices | awk 'NR > 1 && $2 == "device" { print $1 }')
if [[ -n "$SERIAL" ]]; then
  if ! printf '%s\n' "${devices[@]}" | grep -Fxq -- "$SERIAL"; then
    printf 'ADB device %s is not connected and authorized.\n' "$SERIAL" >&2
    exit 1
  fi
elif ((${#devices[@]} != 1)); then
  printf 'Expected exactly one authorized ADB device, found %s. Use --serial when needed.\n' \
    "${#devices[@]}" >&2
  printf '%s\n' "${devices[@]}" >&2
  exit 1
else
  SERIAL="${devices[0]}"
  adb_args=(-s "$SERIAL")
fi

if ! timeout 5 adb "${adb_args[@]}" shell pm path "$PACKAGE_NAME" >/dev/null 2>&1; then
  cat >&2 <<EOF
The debug package $PACKAGE_NAME is not installed on the selected phone.
Install the current PR #88 Android debug build first, then rerun this command.
EOF
  exit 1
fi

if pgrep -x ostudy >/dev/null 2>&1; then
  cat >&2 <<'EOF'
A desktop ostudy process is already running. Close it before starting this
capture so the diagnostic command can own and record one exact desktop session.
EOF
  exit 1
fi

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
OUT_DIR="$OUTPUT_ROOT/$stamp"
mkdir -p "$OUT_DIR"

printf 'oStudy USB Sync physical diagnostics\n'
printf '===================================\n'
printf 'repository: %s\n' "$ROOT_DIR"
printf 'git head:   %s\n' "$(git rev-parse HEAD 2>/dev/null || printf unknown)"
printf 'adb serial: %s\n' "$SERIAL"
printf 'output:     %s\n\n' "$OUT_DIR"

adb_capture() {
  local output="$1"
  shift
  timeout 10 adb "${adb_args[@]}" "$@" >"$output" 2>&1 || true
}

linux_usb_snapshot() {
  if command -v lsusb >/dev/null 2>&1; then
    lsusb
    return
  fi

  local node vendor product manufacturer model serial
  for node in /sys/bus/usb/devices/*; do
    [[ -f "$node/idVendor" && -f "$node/idProduct" ]] || continue
    vendor="$(cat "$node/idVendor" 2>/dev/null || true)"
    product="$(cat "$node/idProduct" 2>/dev/null || true)"
    manufacturer="$(cat "$node/manufacturer" 2>/dev/null || true)"
    model="$(cat "$node/product" 2>/dev/null || true)"
    serial="$(cat "$node/serial" 2>/dev/null || true)"
    printf '%s:%s %s %s serial=%s sysfs=%s\n' \
      "$vendor" "$product" "$manufacturer" "$model" "$serial" "${node##*/}"
  done | sort
}

capture_android_snapshot() {
  local label="$1"
  adb_capture "$OUT_DIR/android-$label-getprop.txt" shell getprop
  adb_capture "$OUT_DIR/android-$label-dumpsys-usb.txt" shell dumpsys usb
  adb_capture "$OUT_DIR/android-$label-package.txt" shell dumpsys package "$PACKAGE_NAME"
  adb_capture "$OUT_DIR/android-$label-activity.txt" shell dumpsys activity activities
  adb_capture "$OUT_DIR/android-$label-features.txt" shell pm list features
  adb_capture "$OUT_DIR/android-$label-resolve-accessory.txt" shell cmd package query-activities \
    --brief -a android.hardware.usb.action.USB_ACCESSORY_ATTACHED
}

print_android_snapshot_summary() {
  local label="$1"
  printf '\n[android snapshot: %s]\n' "$label"
  printf 'USB accessory feature: '
  if grep -Fq 'android.hardware.usb.accessory' "$OUT_DIR/android-$label-features.txt"; then
    printf 'present\n'
  else
    printf 'NOT FOUND\n'
  fi
  grep -E '^\[(sys\.usb\.(config|state)|persist\.sys\.usb\.config)\]:' \
    "$OUT_DIR/android-$label-getprop.txt" || true
  printf '%s\n' '-- installed package USB accessory handler --'
  grep -n -B 3 -A 10 -E 'USB_ACCESSORY_ATTACHED|ostudy_usb_accessory_filter' \
    "$OUT_DIR/android-$label-package.txt" | head -n 80 || true
  printf '%s\n' '-- Android USB service highlights --'
  grep -i -E 'accessory|current.*function|data.?role|power.?role|connected|configured|port' \
    "$OUT_DIR/android-$label-dumpsys-usb.txt" | head -n 120 || true
}

capture_linux_snapshot() {
  local label="$1"
  linux_usb_snapshot >"$OUT_DIR/linux-$label-usb.txt" 2>&1 || true
  if command -v lsusb >/dev/null 2>&1; then
    lsusb -t >"$OUT_DIR/linux-$label-usb-tree.txt" 2>&1 || true
  fi
  {
    printf '%s\n' '/etc/udev/rules.d/70-ostudy-android-usb.rules'
    ls -l /etc/udev/rules.d/70-ostudy-android-usb.rules 2>&1 || true
    printf '\n%s\n' '/usr/lib/udev/rules.d/70-ostudy-android-usb.rules'
    ls -l /usr/lib/udev/rules.d/70-ostudy-android-usb.rules 2>&1 || true
  } >"$OUT_DIR/linux-$label-udev-rules.txt"
}

capture_linux_snapshot before
capture_android_snapshot before
printf '[linux USB before]\n'
cat "$OUT_DIR/linux-before-usb.txt"
print_android_snapshot_summary before

printf '\n[phone]\n'
printf 'model:   '
timeout 5 adb "${adb_args[@]}" shell getprop ro.product.model | tr -d '\r' || true
printf 'android: '
timeout 5 adb "${adb_args[@]}" shell getprop ro.build.version.release | tr -d '\r' || true
printf 'sdk:     '
timeout 5 adb "${adb_args[@]}" shell getprop ro.build.version.sdk | tr -d '\r' || true

printf '\nLaunching oStudy on Android so its USB plugin and log messages are live...\n'
timeout 8 adb "${adb_args[@]}" shell monkey -p "$PACKAGE_NAME" \
  -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1 || true

background_pids=()
desktop_pid=""

cleanup() {
  local status=$?
  trap - EXIT INT TERM
  set +e
  for pid in "${background_pids[@]}"; do
    kill "$pid" >/dev/null 2>&1 || true
  done
  if [[ -n "$desktop_pid" ]]; then
    kill -- "-$desktop_pid" >/dev/null 2>&1 || kill "$desktop_pid" >/dev/null 2>&1 || true
  fi
  wait >/dev/null 2>&1 || true
  exit "$status"
}
trap cleanup EXIT INT TERM

monitor_linux_usb() {
  local previous="" current
  while true; do
    current="$(linux_usb_snapshot 2>/dev/null || true)"
    if [[ "$current" != "$previous" ]]; then
      printf '[%s] Linux USB enumeration changed\n' "$(date -Ins)"
      printf '%s\n' "$current"
      previous="$current"
    fi
    sleep 0.20
  done
}

monitor_android_usb_state() {
  local previous="" current
  while true; do
    current="$(
      timeout 3 adb "${adb_args[@]}" shell \
        'printf "sys.usb.state="; getprop sys.usb.state; printf "sys.usb.config="; getprop sys.usb.config; printf "persist.sys.usb.config="; getprop persist.sys.usb.config' \
        2>&1 | tr -d '\r' || printf 'ADB_UNAVAILABLE'
    )"
    if [[ "$current" != "$previous" ]]; then
      printf '[%s] Android USB properties changed\n%s\n' "$(date -Ins)" "$current"
      previous="$current"
    fi
    sleep 0.35
  done
}

monitor_android_logcat() {
  while true; do
    if timeout 3 adb "${adb_args[@]}" get-state >/dev/null 2>&1; then
      adb "${adb_args[@]}" logcat -v threadtime -T 10 2>&1 || true
    else
      printf '%s ADB temporarily unavailable during USB transition\n' "$(date -Ins)"
    fi
    sleep 0.5
  done
}

monitor_linux_usb \
  > >(tee "$OUT_DIR/linux-usb-timeline.txt" | sed -u 's/^/[linux-usb] /') 2>&1 &
background_pids+=("$!")

monitor_android_usb_state \
  > >(tee "$OUT_DIR/android-usb-state-timeline.txt" | sed -u 's/^/[android-usb] /') 2>&1 &
background_pids+=("$!")

monitor_android_logcat \
  | grep --line-buffered -i -E \
    'ostudy|USB_ACCESSORY|UsbAccessory|UsbDeviceManager|UsbHostManager|UsbPortManager|UsbService|UsbPermission|android\.hardware\.usb|accessory mode' \
  | tee "$OUT_DIR/android-usb-logcat.txt" \
  | sed -u 's/^/[android-log] /' &
background_pids+=("$!")

if command -v udevadm >/dev/null 2>&1; then
  udevadm monitor --kernel --udev --property --subsystem-match=usb 2>&1 \
    | grep --line-buffered -E \
      '^(KERNEL|UDEV)|^(ACTION|DEVPATH|PRODUCT|BUSNUM|DEVNUM|ID_VENDOR_ID|ID_MODEL_ID)=' \
    | tee "$OUT_DIR/linux-udev-timeline.txt" \
    | sed -u 's/^/[udev] /' &
  background_pids+=("$!")
fi

if command -v journalctl >/dev/null 2>&1; then
  journalctl -k -f -n 0 2>/dev/null \
    | grep --line-buffered -i -E 'usb|xhci|type-c|typec' \
    | tee "$OUT_DIR/linux-kernel-usb.txt" \
    | sed -u 's/^/[kernel] /' &
  background_pids+=("$!")
fi

printf '\nLaunching a fresh desktop oStudy process under the same capture...\n'
if command -v setsid >/dev/null 2>&1; then
  (
    exec setsid env OSTUDY_LOG="${OSTUDY_LOG:-info}" "$ROOT_DIR/run-app.sh"
  ) > >(tee "$OUT_DIR/desktop-app.txt" | sed -u 's/^/[desktop] /') 2>&1 &
else
  (
    exec env OSTUDY_LOG="${OSTUDY_LOG:-info}" "$ROOT_DIR/run-app.sh"
  ) > >(tee "$OUT_DIR/desktop-app.txt" | sed -u 's/^/[desktop] /') 2>&1 &
fi
desktop_pid="$!"

cat <<'INSTRUCTIONS'

======================================================================
CAPTURE IS LIVE
======================================================================

Keep the phone UNLOCKED and USB debugging enabled for this diagnostic pass.
Do not change Android's USB preference while the attempt is in progress.

In the desktop app that just opened:
  1. Open Settings -> Backup/Sync -> Device synchronization.
  2. Select the desired categories.
  3. Click USB Sync exactly once.
  4. If Android shows an oStudy USB/accessory permission or app chooser, accept it.
  5. Wait until the desktop shows success or a failure message.

INSTRUCTIONS

if ((CAPTURE_SECONDS > 0)); then
  printf 'Capturing for %s seconds...\n' "$CAPTURE_SECONDS"
  sleep "$CAPTURE_SECONDS"
else
  printf 'When that one attempt has finished, return to this terminal and press Enter.\n'
  read -r _
fi

printf '\nStopping live monitors and collecting the final snapshots...\n'
for pid in "${background_pids[@]}"; do
  kill "$pid" >/dev/null 2>&1 || true
done
background_pids=()

capture_linux_snapshot after
capture_android_snapshot after

# Existing Android diagnostics are intentionally app-private and redacted. ADB
# is used only because this is the explicit debug capture requested by the user.
bash "$SCRIPT_DIR/android-diagnostics.sh" --serial "$SERIAL" \
  --output "$OUT_DIR/android-ostudy.jsonl" >/dev/null 2>&1 || true

printf '\n[linux USB after]\n'
cat "$OUT_DIR/linux-after-usb.txt"
print_android_snapshot_summary after

handler_declared=no
if grep -Fq 'USB_ACCESSORY_ATTACHED' "$OUT_DIR/android-after-package.txt"; then
  handler_declared=yes
fi

aoa_seen=no
if grep -Eiq '18d1:(2d00|2d01)|18d1/2d0[01]|PRODUCT=18d1/2d0[01]/' \
  "$OUT_DIR/linux-usb-timeline.txt" "$OUT_DIR/linux-udev-timeline.txt" 2>/dev/null; then
  aoa_seen=yes
fi

android_accessory_state_seen=no
if grep -Eiq 'accessory(,adb)?|adb,accessory' "$OUT_DIR/android-usb-state-timeline.txt" 2>/dev/null; then
  android_accessory_state_seen=yes
fi

android_ostudy_usb_log_seen=no
if grep -Eiq 'ostudy.*usb|usb.*ostudy|USB_ACCESSORY' "$OUT_DIR/android-usb-logcat.txt" 2>/dev/null; then
  android_ostudy_usb_log_seen=yes
fi

permission_request_seen=no
if grep -Fqi 'Requesting Android permission for oStudy USB Sync accessory' \
  "$OUT_DIR/android-usb-logcat.txt" 2>/dev/null; then
  permission_request_seen=yes
fi

desktop_result="no terminal sync result found"
if grep -Fq 'device_sync_usb_device_not_ready' "$OUT_DIR/desktop-app.txt" 2>/dev/null; then
  desktop_result="AOA re-enumeration timeout (device_sync_usb_device_not_ready)"
elif grep -Fq 'The Android USB device was disconnected' "$OUT_DIR/desktop-app.txt" 2>/dev/null; then
  desktop_result="AOA stream opened, then USB device disconnected"
elif grep -Eq 'device_sync_session_ended.*state=completed' "$OUT_DIR/desktop-app.txt" 2>/dev/null; then
  desktop_result="sync completed"
elif grep -Eq 'device_sync_session_ended.*state=failed' "$OUT_DIR/desktop-app.txt" 2>/dev/null; then
  desktop_result="sync failed after session creation"
fi

printf '\nDiagnostic phase summary\n'
printf '%s\n' '------------------------'
printf 'Installed debug APK declares USB accessory handler: %s\n' "$handler_declared"
printf 'Linux observed AOA VID:PID 18d1:2d00/2d01:       %s\n' "$aoa_seen"
printf 'Android reported accessory USB function state:     %s\n' "$android_accessory_state_seen"
printf 'Android emitted oStudy/accessory-related logcat:   %s\n' "$android_ostudy_usb_log_seen"
printf 'oStudy explicit Android permission request seen:   %s\n' "$permission_request_seen"
printf 'Desktop result: %s\n' "$desktop_result"

archive="$OUT_DIR.tar.gz"
tar -C "$(dirname -- "$OUT_DIR")" -czf "$archive" "$(basename -- "$OUT_DIR")"
printf '\nCapture directory: %s\n' "$OUT_DIR"
printf 'Uploadable archive: %s\n' "$archive"
printf '%s\n' 'The archive contains USB/oStudy diagnostics only; general phone logcat is filtered out.'

# End the diagnostic desktop process after all snapshots are complete. This
# keeps future manual runs separate from the captured process/session.
if [[ -n "$desktop_pid" ]]; then
  kill -- "-$desktop_pid" >/dev/null 2>&1 || kill "$desktop_pid" >/dev/null 2>&1 || true
  desktop_pid=""
fi

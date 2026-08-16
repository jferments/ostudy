#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

MODE=debug
TARGET=aarch64
BUILD=1
SERIAL=""

usage() {
  cat <<'USAGE'
Usage: ./scripts/android-install.sh [options]

Build and install an ostudy APK on one connected Android device.

Options:
  --debug                 Install a debuggable build (default).
  --release               Install a release APK produced with configured signing.
  --target <abi|all>      aarch64, armv7, i686, x86_64, or all (default: aarch64).
  --serial <adb-serial>   Select one device when more than one is connected.
  --no-build              Install the newest existing matching APK.
  -h, --help              Show this help.
USAGE
}

while (($#)); do
  case "$1" in
    --debug) MODE=debug ;;
    --release) MODE=release ;;
    --target)
      shift
      TARGET="${1:?--target requires an ABI}"
      ;;
    --serial)
      shift
      SERIAL="${1:?--serial requires an adb device serial}"
      ;;
    --no-build) BUILD=0 ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown Android install option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

case "$TARGET" in
  aarch64|armv7|i686|x86_64|all) ;;
  *) printf 'Unsupported Android target: %s\n' "$TARGET" >&2; exit 2 ;;
esac

cd "$ROOT_DIR"
bash ./scripts/android-doctor.sh

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
  printf 'Expected exactly one connected Android device, found %s. Use --serial when needed.\n' \
    "${#devices[@]}" >&2
  printf '%s\n' "${devices[@]}" >&2
  exit 1
fi

if ((BUILD == 1)); then
  build_args=("--$MODE" --apk --target "$TARGET")
  bash ./scripts/android-build.sh "${build_args[@]}"
fi

variant="$MODE"
mapfile -t candidates < <(
  find src-tauri/gen/android/app/build/outputs/apk \
    -type f -name "*${variant}*.apk" -printf '%T@ %p\n' 2>/dev/null \
    | sort -nr \
    | awk '{ $1=""; sub(/^ /, ""); print }'
)
if ((${#candidates[@]} == 0)); then
  printf 'No %s APK was found. Run without --no-build first.\n' "$MODE" >&2
  exit 1
fi
apk="${candidates[0]}"

bash ./scripts/verify-android-package.sh "$apk" "$MODE"
printf 'Installing %s...\n' "$apk"
adb "${adb_args[@]}" install -r "$apk"

package_name="io.github.jferments.ostudy"
adb "${adb_args[@]}" shell monkey -p "$package_name" -c android.intent.category.LAUNCHER 1 >/dev/null
printf 'OK   Installed and launched ostudy on %s.\n' "${SERIAL:-${devices[0]} }"

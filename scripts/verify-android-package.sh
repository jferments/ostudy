#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

APK="${1:-}"
MODE="${2:-debug}"
if [[ -z "$APK" || ! -f "$APK" ]]; then
  printf 'Usage: ./scripts/verify-android-package.sh <apk> [debug|release]\n' >&2
  exit 2
fi
case "$MODE" in debug|release) ;; *) printf 'Mode must be debug or release.\n' >&2; exit 2 ;; esac

AAPT="$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS_VERSION/aapt"
ZIPALIGN="$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS_VERSION/zipalign"
READELF="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-readelf"
for tool in "$AAPT" "$ZIPALIGN" "$READELF" unzip grep awk; do
  if [[ ! -x "$tool" ]] && ! command -v "$tool" >/dev/null 2>&1; then
    printf 'Required Android package verifier is missing: %s\n' "$tool" >&2
    exit 1
  fi
done

badging="$($AAPT dump badging "$APK")"
permissions="$($AAPT dump permissions "$APK")"
listing="$(unzip -Z1 "$APK")"

expected_package="io.github.jferments.ostudy"
[[ "$MODE" == debug ]] && expected_package+=".dev"
actual_package="$(sed -n "s/^package: name='\([^']*\)'.*/\1/p" <<< "$badging")"
min_sdk="$(sed -n "s/^sdkVersion:'\([^']*\)'.*/\1/p" <<< "$badging")"
target_sdk="$(sed -n "s/^targetSdkVersion:'\([^']*\)'.*/\1/p" <<< "$badging")"

[[ "$actual_package" == "$expected_package" ]] || {
  printf 'Unexpected Android package: expected %s, found %s\n' "$expected_package" "$actual_package" >&2
  exit 1
}
[[ "$min_sdk" == "$ANDROID_MIN_SDK" ]] || {
  printf 'Unexpected min SDK: expected %s, found %s\n' "$ANDROID_MIN_SDK" "$min_sdk" >&2
  exit 1
}
[[ "$target_sdk" == "$ANDROID_TARGET_SDK" ]] || {
  printf 'Unexpected target SDK: expected %s, found %s\n' "$ANDROID_TARGET_SDK" "$target_sdk" >&2
  exit 1
}

for forbidden_permission in \
  android.permission.READ_EXTERNAL_STORAGE \
  android.permission.WRITE_EXTERNAL_STORAGE \
  android.permission.MANAGE_EXTERNAL_STORAGE \
  android.permission.REQUEST_INSTALL_PACKAGES; do
  if grep -Fq "$forbidden_permission" <<< "$permissions"; then
    printf 'Forbidden broad Android permission found: %s\n' "$forbidden_permission" >&2
    exit 1
  fi
done

for forbidden_payload in pdfium tesseract tessdata liblept; do
  if grep -Fiq "$forbidden_payload" <<< "$listing"; then
    printf 'Document-processing payload must not be bundled during A1-A3: %s\n' "$forbidden_payload" >&2
    exit 1
  fi
done

"$ZIPALIGN" -c -P 16 -v 4 "$APK" >/dev/null

temporary="$(mktemp -d)"
cleanup() { rm -rf "$temporary"; }
trap cleanup EXIT
unzip -q "$APK" 'lib/*/*.so' -d "$temporary"
mapfile -t libraries < <(find "$temporary/lib" -type f -name '*.so' -print | sort)
if ((${#libraries[@]} == 0)); then
  printf 'The APK contains no native ostudy libraries.\n' >&2
  exit 1
fi
for library in "${libraries[@]}"; do
  while read -r alignment_literal; do
    [[ -n "$alignment_literal" ]] || continue
    alignment=$((alignment_literal))
    if ((alignment < 0x4000)); then
      printf 'Native library is not 16 KiB aligned: %s (LOAD alignment 0x%x)\n' \
        "${library#"$temporary/"}" "$alignment" >&2
      exit 1
    fi
  done < <("$READELF" -lW "$library" | awk '$1 == "LOAD" { print $NF }')
done

# The build wrapper verifies the smoke bridge against the freshly generated
# Vite distribution before Tauri packages it. Tauri may compress/embed web
# assets, so absence of a JavaScript literal in an unpacked APK is not proof
# that the bridge is absent. Keep this release scan only as defense in depth.
if [[ "$MODE" == release ]]; then
  unzip -q "$APK" -d "$temporary/release-tree"
  if grep -R -a -q '__OSTUDY_ANDROID_TEST__' "$temporary/release-tree"; then
    printf 'Release APK exposes the emulator-only Android test bridge marker.\n' >&2
    exit 1
  fi
fi

printf 'OK   Android APK package=%s minSdk=%s targetSdk=%s nativeLibraries=%s zipAlignment=16KiB\n' \
  "$actual_package" "$min_sdk" "$target_sdk" "${#libraries[@]}"

#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-qualification-gates.sh"

DESTINATION="${1:-}"
if [[ -z "$DESTINATION" ]]; then
  printf 'Usage: ./scripts/stage-android-smoke-artifacts.sh DESTINATION\n' >&2
  exit 2
fi

cd "$ROOT_DIR"
app_apk="$(ostudy_android_find_app_apk)"
test_apk="$(ostudy_android_find_test_apk)"
if [[ -z "$app_apk" || ! -f "$app_apk" || -z "$test_apk" || ! -f "$test_apk" ]]; then
  printf 'Android smoke application/instrumentation APKs are not both available.\n' >&2
  exit 1
fi

rm -rf -- "$DESTINATION"
mkdir -p "$DESTINATION"
cp -- "$app_apk" "$DESTINATION/ostudy-smoke-app.apk"
cp -- "$test_apk" "$DESTINATION/ostudy-smoke-test.apk"
(
  cd "$DESTINATION"
  sha256sum ostudy-smoke-app.apk ostudy-smoke-test.apk > SHA256SUMS
)
printf 'source_app=%s\nsource_test=%s\n' \
  "${app_apk#"$ROOT_DIR/"}" \
  "${test_apk#"$ROOT_DIR/"}" \
  > "$DESTINATION/source-paths.txt"
printf 'OK   Staged deterministic Android smoke APK pair in %s.\n' "$DESTINATION"

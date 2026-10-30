#!/usr/bin/env bash
set -Eeuo pipefail

# Signs the unsigned release APK with oStudy's long-lived release key and
# proves the result: 16 KiB zip alignment, a valid v2/v3 signature, and a
# signing certificate whose SHA-256 matches the one committed in
# src-tauri/packaging/android/release-certificate.sha256. Every later APK must
# carry the same certificate, or Android refuses it as an update.

usage() {
  cat <<'USAGE'
Usage: ./scripts/sign-android-release.sh UNSIGNED_APK SIGNED_APK

Reads the release key from the environment; nothing secret is passed on the
command line or written to the repository:
  OSTUDY_ANDROID_KEYSTORE           path to the keystore (outside the checkout)
  OSTUDY_ANDROID_KEYSTORE_PASSWORD  its password
  OSTUDY_ANDROID_KEY_ALIAS          the key's alias (default: ostudy)
USAGE
}

[[ $# -eq 2 ]] || { usage >&2; exit 2; }
UNSIGNED="$1"
SIGNED="$2"

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
CERTIFICATE_FILE="$ROOT_DIR/src-tauri/packaging/android/release-certificate.sha256"

ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
BUILD_TOOLS="${OSTUDY_ANDROID_BUILD_TOOLS:-$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS_VERSION}"
ZIPALIGN="$BUILD_TOOLS/zipalign"
APKSIGNER="$BUILD_TOOLS/apksigner"
for tool in "$ZIPALIGN" "$APKSIGNER"; do
  [[ -x "$tool" ]] || { printf 'ERROR: %s is missing; run pnpm android:install-deps.\n' "$tool" >&2; exit 1; }
done

[[ -f "$UNSIGNED" ]] || { printf 'ERROR: no APK at %s.\n' "$UNSIGNED" >&2; exit 1; }
KEYSTORE="${OSTUDY_ANDROID_KEYSTORE:-}"
ALIAS="${OSTUDY_ANDROID_KEY_ALIAS:-ostudy}"
[[ -n "$KEYSTORE" && -f "$KEYSTORE" ]] || {
  printf 'ERROR: set OSTUDY_ANDROID_KEYSTORE to the release keystore file.\n' >&2
  exit 1
}
[[ -n "${OSTUDY_ANDROID_KEYSTORE_PASSWORD:-}" ]] || {
  printf 'ERROR: set OSTUDY_ANDROID_KEYSTORE_PASSWORD (read -rs OSTUDY_ANDROID_KEYSTORE_PASSWORD; export it).\n' >&2
  exit 1
}
case "$(realpath "$KEYSTORE")" in
  "$ROOT_DIR"/*) printf 'ERROR: keep the release keystore outside the checkout.\n' >&2; exit 1 ;;
esac
[[ -f "$CERTIFICATE_FILE" ]] || {
  printf 'ERROR: %s is missing; commit the release certificate SHA-256 first.\n' "${CERTIFICATE_FILE#"$ROOT_DIR/"}" >&2
  exit 1
}
expected="$(tr -d ' :\n' < "$CERTIFICATE_FILE" | tr '[:upper:]' '[:lower:]')"

work="$(mktemp -d -t ostudy-android-sign.XXXXXXXX)"
trap 'rm -rf -- "$work"' EXIT
"$ZIPALIGN" -f -P 16 4 "$UNSIGNED" "$work/aligned.apk"
"$APKSIGNER" sign \
  --ks "$KEYSTORE" --ks-key-alias "$ALIAS" --ks-pass env:OSTUDY_ANDROID_KEYSTORE_PASSWORD \
  --out "$work/signed.apk" "$work/aligned.apk"

"$ZIPALIGN" -c -P 16 4 "$work/signed.apk" >/dev/null
report="$("$APKSIGNER" verify --verbose --print-certs "$work/signed.apk")"
grep -q '^Verified using v2 scheme (APK Signature Scheme v2): true' <<< "$report" || {
  printf 'ERROR: the signed APK has no valid v2 signature.\n%s\n' "$report" >&2
  exit 1
}
actual="$(sed -n 's/^Signer #1 certificate SHA-256 digest: //p' <<< "$report" | tr -d ' :' | tr '[:upper:]' '[:lower:]')"
[[ -n "$actual" && "$actual" == "$expected" ]] || {
  printf 'ERROR: signed with certificate %s, but releases use %s.\n' "${actual:-none}" "$expected" >&2
  printf 'Is OSTUDY_ANDROID_KEYSTORE the oStudy release key?\n' >&2
  exit 1
}

mkdir -p "$(dirname -- "$SIGNED")"
cp -- "$work/signed.apk" "$SIGNED"
printf 'OK   Signed %s with the release certificate %s\n' "$SIGNED" "$actual"

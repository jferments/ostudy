#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"

DEB="${1:-}"
BUILT_GUI="${2:-}"
[[ -n "$DEB" && -f "$DEB" && -n "$BUILT_GUI" && -f "$BUILT_GUI" ]] || {
  printf 'Usage: ./scripts/verify-deb-package.sh path/to/ostudy.deb path/to/built/ostudy\n' >&2
  exit 2
}
command -v dpkg-deb >/dev/null 2>&1 || {
  printf 'ERROR: dpkg-deb is required to inspect the Debian package.\n' >&2
  exit 1
}

PACKAGE="$(dpkg-deb -f "$DEB" Package)"
[[ "$PACKAGE" == "ostudy" ]] || {
  printf 'ERROR: expected Debian package name ostudy, found %s.\n' "$PACKAGE" >&2
  exit 1
}
DEPENDS="$(dpkg-deb -f "$DEB" Depends)"
printf '%s\n' "$DEPENDS" | grep -Eq '(^|,)[[:space:]]*libusb-1\.0-0([[:space:]]*\([^)]*\))?([[:space:]]*,|$)' || {
  printf 'ERROR: Debian package is missing required dependency libusb-1.0-0. Depends: %s\n' "$DEPENDS" >&2
  exit 1
}
# Startup builds the AI HTTP client, which needs the system certificate store.
printf '%s\n' "$DEPENDS" | grep -Eq '(^|,)[[:space:]]*ca-certificates([[:space:]]*\([^)]*\))?([[:space:]]*,|$)' || {
  printf 'ERROR: Debian package is missing required dependency ca-certificates. Depends: %s\n' "$DEPENDS" >&2
  exit 1
}

TMP="$(mktemp -d -t ostudy-deb-policy.XXXXXXXX)"
cleanup() { rm -rf -- "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/control"

dpkg-deb --contents "$DEB" > "$TMP/contents.txt"
dpkg-deb --control "$DEB" "$TMP/control"
mkdir -p "$TMP/data"
dpkg-deb --extract "$DEB" "$TMP/data"

python3 "$SCRIPT_DIR/verify-packaged-launcher.py" \
  "$TMP/data" "$BUILT_GUI" "$ROOT_DIR/src-tauri/Cargo.toml"

if grep -Eiq '(^|[[:space:]])\.?/(home|root)/|\.local/share' "$TMP/contents.txt"; then
  printf 'ERROR: Debian package owns a user/home application-data path.\n' >&2
  grep -Ei '(^|[[:space:]])\.?/(home|root)/|\.local/share' "$TMP/contents.txt" >&2 || true
  exit 1
fi

for script in preinst postinst prerm postrm; do
  path="$TMP/control/$script"
  [[ -f "$path" ]] || continue
  if grep -Eiq 'XDG_DATA_HOME|\.local/share|/home/|\$HOME|\${HOME' "$path"; then
    printf 'ERROR: Debian maintainer script %s references user learner-data locations.\n' "$script" >&2
    exit 1
  fi
done

PACKAGED_UDEV="$TMP/data/usr/lib/udev/rules.d/70-ostudy-android-usb.rules"
SOURCE_UDEV="$ROOT_DIR/src-tauri/packaging/linux/70-ostudy-android-usb.rules"
[[ -f "$PACKAGED_UDEV" ]] || {
  printf 'ERROR: Debian package is missing the reviewed Android USB udev rule.\n' >&2
  grep -Ei 'udev|rules\.d' "$TMP/contents.txt" >&2 || true
  exit 1
}
cmp -s "$SOURCE_UDEV" "$PACKAGED_UDEV" || {
  printf 'ERROR: packaged Android USB udev rule does not match the reviewed source file.\n' >&2
  exit 1
}

PACKAGED_PDFIUM="$TMP/data/usr/lib/ostudy/native/pdfium/libpdfium.so"
SOURCE_PDFIUM="$ROOT_DIR/src-tauri/resources/native/pdfium/libpdfium.so"
[[ -f "$PACKAGED_PDFIUM" ]] || {
  printf 'ERROR: Debian package is missing the staged PDFium runtime.\n' >&2
  grep -Ei 'pdfium|usr/lib/ostudy' "$TMP/contents.txt" >&2 || true
  exit 1
}
[[ -f "$SOURCE_PDFIUM" ]] || {
  printf 'ERROR: verified source PDFium runtime is missing before package inspection.\n' >&2
  exit 1
}
cmp -s "$SOURCE_PDFIUM" "$PACKAGED_PDFIUM" || {
  printf 'ERROR: packaged PDFium runtime does not match the verified staged library.\n' >&2
  exit 1
}

printf 'OK   Debian package contains the reviewed udev rule, verified PDFium runtime, libusb dependency, and no user-owned learner-data paths.\n'

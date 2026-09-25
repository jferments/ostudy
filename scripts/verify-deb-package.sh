#!/usr/bin/env bash
set -Eeuo pipefail

DEB="${1:-}"
[[ -n "$DEB" && -f "$DEB" ]] || {
  printf 'Usage: ./scripts/verify-deb-package.sh path/to/ostudy.deb\n' >&2
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

TMP="$(mktemp -d -t ostudy-deb-policy.XXXXXXXX)"
cleanup() { rm -rf -- "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/control"

dpkg-deb --contents "$DEB" > "$TMP/contents.txt"
dpkg-deb --control "$DEB" "$TMP/control"

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

grep -Fq './usr/lib/udev/rules.d/70-ostudy-android-usb.rules' "$TMP/contents.txt" || {
  printf 'ERROR: Debian package is missing the reviewed Android USB udev rule.\n' >&2
  exit 1
}

printf 'OK   Debian package owns only system/application files; user learner data is outside package ownership.\n'

#!/usr/bin/env bash
set -Eeuo pipefail

# Turn the verified production .deb into a plain .tar.gz that runs from any
# folder: bin/ostudy finds PDFium in ../lib/ostudy the same way the installed
# package does. The system still supplies WebKitGTK 4.1, GTK 3 and libusb.

DEB="${1:-}"
BUILT_GUI="${2:-}"
OUTPUT_DIR="${3:-}"
[[ -f "$DEB" && -f "$BUILT_GUI" && -n "$OUTPUT_DIR" ]] || {
  printf 'Usage: ./scripts/package-linux-tarball.sh path/to/ostudy.deb path/to/built/ostudy OUTPUT_DIR\n' >&2
  exit 2
}

NAME="$(basename -- "$DEB" .deb)"
WORK="$(mktemp -d -t ostudy-tarball.XXXXXXXX)"
cleanup() { rm -rf -- "$WORK"; }
trap cleanup EXIT
dpkg-deb --extract "$DEB" "$WORK/deb"
ROOT="$WORK/$NAME"
mkdir -p "$ROOT/bin"
# The unpatched build, not the .deb's copy, so it does not claim to be a .deb install.
install -m 0755 -- "$BUILT_GUI" "$ROOT/bin/ostudy"
mkdir -p "$ROOT/lib"
cp -a -- "$WORK/deb/usr/lib/ostudy" "$ROOT/lib/ostudy"
cp -a -- "$WORK/deb/usr/share" "$ROOT/share"
install -D -m 0644 -- "$WORK/deb/usr/lib/udev/rules.d/70-ostudy-android-usb.rules" \
  "$ROOT/udev/70-ostudy-android-usb.rules"
cat > "$ROOT/README.txt" <<'README'
oStudy for Linux (portable folder)

Run it from anywhere:

    ./bin/ostudy

Your library is kept in ~/.local/share/io.github.jferments.ostudy, the same
place the .deb and .rpm packages use, so deleting this folder never deletes it.

oStudy needs these system libraries, which most desktops already have:

  Debian/Ubuntu:  sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0 libusb-1.0-0 ca-certificates
  Fedora:         sudo dnf install webkit2gtk4.1 gtk3 libusb1 ca-certificates
  openSUSE:       sudo zypper install libwebkit2gtk-4_1-0 gtk3 libusb-1_0-0 ca-certificates
  Arch:           sudo pacman -S webkit2gtk-4.1 gtk3 libusb ca-certificates

To sync with an Android phone over USB, copy udev/70-ostudy-android-usb.rules
to /etc/udev/rules.d/ once (sudo), then reconnect the phone.
README
mkdir -p "$OUTPUT_DIR"
tar -C "$WORK" --owner=0 --group=0 -czf "$OUTPUT_DIR/$NAME.tar.gz" "$NAME"
printf 'OK   Portable tarball: %s\n' "$OUTPUT_DIR/$NAME.tar.gz"

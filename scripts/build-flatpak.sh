#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

APP_ID="io.github.jferments.ostudy"
FLATPAK_DIR="$ROOT_DIR/src-tauri/packaging/linux/flatpak"
MANIFEST="$FLATPAK_DIR/$APP_ID.yml"
OUTPUT_DIR="$ROOT_DIR/.artifacts/flatpak"
DEB=""
INSTALL=0
CONTAINER=0
ENGINE="${OSTUDY_CONTAINER_ENGINE:-}"
CACHE_VOLUME="ostudy-flatpak-cache"

usage() {
  cat <<'USAGE'
Usage: ./scripts/build-flatpak.sh [--deb PATH] [--output DIRECTORY] [--install | --container]

Build the oStudy Flatpak from a verified production .deb and write a
single-file bundle (.flatpak). Without --deb it uses the package from
./scripts/ostudy-dev package-deb (.artifacts/linux-package/). --install also
installs it for the current user.

--container builds inside a privileged Ubuntu 24.04 container (Docker or
Podman, chosen by OSTUDY_CONTAINER_ENGINE) instead of on the host. The release
runner uses it because its service sandbox stops flatpak-builder's bubblewrap
from setting up a network namespace. The runtime is cached in the container
volume ostudy-flatpak-cache.
USAGE
}

while (($#)); do
  case "$1" in
    --deb) [[ $# -ge 2 ]] || { usage >&2; exit 2; }; DEB="$2"; shift 2 ;;
    --output) [[ $# -ge 2 ]] || { usage >&2; exit 2; }; OUTPUT_DIR="$2"; shift 2 ;;
    --install) INSTALL=1; shift ;;
    --container) CONTAINER=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'ERROR: unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -z "$DEB" ]]; then
  shopt -s nullglob
  candidates=("$ROOT_DIR"/.artifacts/linux-package/*.deb)
  (( ${#candidates[@]} == 1 )) || {
    printf 'ERROR: build the production package first (./scripts/ostudy-dev package-deb) or pass --deb PATH.\n' >&2
    exit 1
  }
  DEB="${candidates[0]}"
fi
[[ -f "$DEB" ]] || { printf 'ERROR: no such package: %s\n' "$DEB" >&2; exit 1; }
[[ "$(dpkg-deb -f "$DEB" Package)" == "ostudy" ]] || {
  printf 'ERROR: %s is not the ostudy package.\n' "$DEB" >&2
  exit 1
}
VERSION="$(dpkg-deb -f "$DEB" Version)"

if (( CONTAINER )); then
  (( ! INSTALL )) || { printf 'ERROR: --install and --container cannot be combined.\n' >&2; exit 2; }
  if [[ -z "$ENGINE" ]]; then
    if command -v podman >/dev/null 2>&1; then ENGINE=podman
    elif command -v docker >/dev/null 2>&1; then ENGINE=docker
    else printf 'ERROR: --container needs Podman or Docker.\n' >&2; exit 1
    fi
  fi
  DEB="$(cd -- "$(dirname -- "$DEB")" && pwd)/$(basename -- "$DEB")"
  mkdir -p "$OUTPUT_DIR"
  OUTPUT_DIR="$(cd -- "$OUTPUT_DIR" && pwd)"
  read -r -a EXTRA_ARGS <<< "${OSTUDY_CONTAINER_ARGS:-}"
  printf '==> Building the Flatpak in ubuntu:24.04 (%s)\n' "$ENGINE"
  # flatpak-builder sandboxes each build step with bubblewrap, which needs
  # namespaces an unprivileged container does not get.
  "$ENGINE" run --rm --privileged \
    -v "$ROOT_DIR:/source:ro" \
    -v "$DEB:/input/ostudy.deb:ro" \
    -v "$OUTPUT_DIR:/output" \
    -v "$CACHE_VOLUME:/cache" \
    -e HOME=/cache/home \
    -e OSTUDY_FLATPAK_WORK_ROOT=/cache \
    "${EXTRA_ARGS[@]}" \
    ubuntu:24.04 bash -Eeuo pipefail -c '
      export DEBIAN_FRONTEND=noninteractive
      apt-get update -qq
      apt-get install -y -qq flatpak flatpak-builder appstream-compose dbus bzip2 xz-utils git patch unzip >/dev/null
      mkdir -p "$HOME" /run/dbus
      export XDG_RUNTIME_DIR=/run/user/0
      mkdir -p -m 700 "$XDG_RUNTIME_DIR"
      dbus-daemon --system --fork
      dbus-run-session -- /source/scripts/build-flatpak.sh --deb /input/ostudy.deb --output /output
      chown -R "$(stat -c %u:%g /output)" /output
    '
  exit 0
fi

for tool in flatpak flatpak-builder; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'ERROR: %s is required. On Ubuntu/Debian: sudo apt-get install flatpak flatpak-builder\n' "$tool" >&2
    exit 1
  }
done

RUNTIME_VERSION="$(sed -n 's/^runtime-version: "\(.*\)"$/\1/p' "$MANIFEST")"
[[ -n "$RUNTIME_VERSION" ]] || { printf 'ERROR: manifest has no runtime-version.\n' >&2; exit 1; }

printf '==> Installing org.gnome.Platform and Sdk %s for this user\n' "$RUNTIME_VERSION"
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user --noninteractive flathub \
  "org.gnome.Platform//$RUNTIME_VERSION" "org.gnome.Sdk//$RUNTIME_VERSION"

# oStudy links WebKitGTK 4.1 (GTK 3). Fail clearly if a runtime update drops it.
flatpak run --command=sh "org.gnome.Platform//$RUNTIME_VERSION" -c \
  'ls /usr/lib/*/libwebkit2gtk-4.1.so.0 >/dev/null 2>&1' || {
  printf 'ERROR: org.gnome.Platform %s does not ship WebKitGTK 4.1; pick a runtime that does.\n' "$RUNTIME_VERSION" >&2
  exit 1
}

# flatpak-builder needs its state, build and repo directories on one
# filesystem, and /tmp is often a separate tmpfs, so work inside the checkout.
WORK_ROOT="${OSTUDY_FLATPAK_WORK_ROOT:-$ROOT_DIR/.artifacts}"
mkdir -p "$WORK_ROOT"
WORK="$(mktemp -d -p "$WORK_ROOT" flatpak-work.XXXXXXXX)"
cleanup() { rm -rf -- "$WORK"; }
trap cleanup EXIT
cp -- "$MANIFEST" "$FLATPAK_DIR/$APP_ID.metainfo.xml" "$WORK/"
cp -- "$DEB" "$WORK/ostudy.deb"

printf '==> Building %s %s\n' "$APP_ID" "$VERSION"
flatpak-builder --user --force-clean --disable-rofiles-fuse \
  --state-dir="$WORK/state" --repo="$WORK/repo" "$WORK/build" "$WORK/$APP_ID.yml"

ARCH="$(flatpak --default-arch)"
mkdir -p "$OUTPUT_DIR"
BUNDLE="$OUTPUT_DIR/ostudy-v$VERSION-linux-$ARCH.flatpak"
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
  "$WORK/repo" "$BUNDLE" "$APP_ID"
(cd "$OUTPUT_DIR" && sha256sum "$(basename -- "$BUNDLE")" > SHA256SUMS.txt)
printf '\nFlatpak bundle: %s\n' "$BUNDLE"

if (( INSTALL )); then
  flatpak install --user --noninteractive --reinstall --bundle "$BUNDLE"
  printf 'Installed. Run it with: flatpak run %s\n' "$APP_ID"
fi

#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

# Linux packages built on the host's own glibc only run on distributions at
# least that new: built on Ubuntu 24.04 they need glibc 2.39 and refuse to
# start on Debian 12 or Ubuntu 22.04. This builds the same verified packages
# inside an Ubuntu 22.04 container (glibc 2.35), plus an AppImage and a plain
# tarball, so one set of files runs on every maintained major distribution.

IMAGE="ubuntu:22.04"
OUTPUT_DIR="$ROOT_DIR/.artifacts/linux-portable"
CACHE_VOLUME="ostudy-linux-portable-cache"
ENGINE="${OSTUDY_CONTAINER_ENGINE:-}"

usage() {
  cat <<'USAGE'
Usage: ./scripts/package-linux-portable.sh [--output DIRECTORY] [--image IMAGE]

Build the production .deb, .rpm, AppImage and .tar.gz for the exact committed
source inside an Ubuntu 22.04 container (Docker or Podman), verify them, and
write them with BUILD-INFO.txt and SHA256SUMS.txt to the output directory
(default .artifacts/linux-portable). Toolchains and Cargo output are cached in
the container volume ostudy-linux-portable-cache between runs.
OSTUDY_CONTAINER_ENGINE selects docker or podman; OSTUDY_CONTAINER_ARGS adds
arguments to the container run (for example proxy settings).
USAGE
}

while (($#)); do
  case "$1" in
    --output) [[ $# -ge 2 ]] || { usage >&2; exit 2; }; OUTPUT_DIR="$2"; shift 2 ;;
    --image) [[ $# -ge 2 ]] || { usage >&2; exit 2; }; IMAGE="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'ERROR: unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -z "$ENGINE" ]]; then
  if command -v podman >/dev/null 2>&1; then ENGINE=podman
  elif command -v docker >/dev/null 2>&1; then ENGINE=docker
  else
    printf 'ERROR: install Podman or Docker (on Ubuntu: sudo apt-get install podman).\n' >&2
    exit 1
  fi
fi

git rev-parse --is-inside-work-tree >/dev/null
git diff --quiet && git diff --cached --quiet || {
  printf 'ERROR: portable packages are built from committed source; commit or stash changes first.\n' >&2
  exit 1
}
mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd -- "$OUTPUT_DIR" && pwd)"

read -r -a EXTRA_ARGS <<< "${OSTUDY_CONTAINER_ARGS:-}"
printf '==> Building portable Linux packages for %s in %s (%s)\n' "$(git rev-parse --short HEAD)" "$IMAGE" "$ENGINE"
"$ENGINE" run --rm \
  -v "$ROOT_DIR:/source:ro" \
  -v "$OUTPUT_DIR:/output" \
  -v "$CACHE_VOLUME:/cache" \
  -e HOME=/cache/home \
  "${EXTRA_ARGS[@]}" \
  "$IMAGE" bash -Eeuo pipefail -c '
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    # Ubuntu 22.04 ships Python 3.10; the package verifiers need tomllib (3.11).
    # The AppImage bundler copies xdg-open and the GStreamer plugins from here.
    apt-get install -y -qq sudo git ca-certificates curl python3.11 xdg-utils file \
      gstreamer1.0-plugins-base gstreamer1.0-plugins-good >/dev/null
    ln -sf /usr/bin/python3.11 /usr/local/bin/python3
    mkdir -p "$HOME"
    git config --global --add safe.directory "*"
    rm -rf /cache/work
    git clone -q /source /cache/work
    cd /cache/work
    ./scripts/setup-ubuntu.sh --no-build > /cache/setup.log 2>&1 || { tail -40 /cache/setup.log; exit 1; }
    export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=/cache/target CARGO_INCREMENTAL=0
    ./scripts/check-linux-package.sh --output /cache/packages
    pnpm tauri build --config src-tauri/tauri.linux.conf.json --bundles appimage --ci -- --locked
    ./scripts/package-linux-tarball.sh /cache/packages/*.deb "$CARGO_TARGET_DIR/release/ostudy" /cache/packages
    cp "$CARGO_TARGET_DIR"/release/bundle/appimage/*.AppImage /cache/packages/
    chown -R "$(stat -c %u:%g /output)" /cache/packages
    cp -a /cache/packages/. /output/
  '

cd "$OUTPUT_DIR"
for appimage in ./*.AppImage; do
  [[ -e "$appimage" ]] || continue
  target="$(sed -n 's/^package=\(.*\)\.deb$/\1.AppImage/p' BUILD-INFO.txt)"
  [[ -n "$target" && "$appimage" != "./$target" ]] && mv -f -- "$appimage" "$target"
  chmod +x -- "$target"
done
printf 'build_image=%s\n' "$IMAGE" >> BUILD-INFO.txt
sha256sum -- *.deb *.rpm *.AppImage *.tar.gz BUILD-INFO.txt > SHA256SUMS.txt
printf '\nPortable Linux packages in %s\n' "$OUTPUT_DIR"
ls -1 "$OUTPUT_DIR"

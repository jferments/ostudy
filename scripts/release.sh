#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/node-toolchain.bash"
ostudy_activate_node_toolchain
export OSTUDY_ROOT_DIR="$ROOT_DIR"
cd "$ROOT_DIR"

VERSION="$(node -p "require('./package.json').version")"
EXPECTED_TAG="v$VERSION"
REQUESTED_TAG="${1:-$EXPECTED_TAG}"
[[ "$REQUESTED_TAG" == "$EXPECTED_TAG" ]] || {
  printf 'ERROR: release tag %s does not match manifest version %s.\n' "$REQUESTED_TAG" "$VERSION" >&2
  exit 1
}

git rev-parse --is-inside-work-tree >/dev/null
git diff --quiet && git diff --cached --quiet || {
  printf 'ERROR: release artifacts must be generated from a clean Git worktree.\n' >&2
  exit 1
}
node "$SCRIPT_DIR/verify-project-manifests.mjs"

"$SCRIPT_DIR/release-check.sh"

case "$(uname -m)" in
  x86_64) ARCH="x86_64" ;;
  *) printf 'ERROR: Linux releases are built on x86_64 only, not %s.\n' "$(uname -m)" >&2; exit 1 ;;
esac

OUT_DIR="$ROOT_DIR/artifacts/$EXPECTED_TAG/linux-$ARCH"
rm -rf -- "$ROOT_DIR/artifacts/$EXPECTED_TAG"
mkdir -p "$OUT_DIR"

# Built in an Ubuntu 22.04 container so the files start on every maintained
# distribution; it qualifies the .deb and .rpm with check-linux-package.sh
# (verify-deb-package.sh, verify-rpm-package.py) before writing anything.
printf '\n==> Building the portable .deb, .rpm, AppImage and tarball\n'
"$SCRIPT_DIR/package-linux-portable.sh" --output "$OUT_DIR"
DEB="$OUT_DIR/ostudy-$EXPECTED_TAG-linux-$ARCH.deb"
[[ -f "$DEB" && -f "$OUT_DIR/ostudy-$EXPECTED_TAG-linux-$ARCH.rpm" && -f "$OUT_DIR/ostudy-$EXPECTED_TAG-linux-$ARCH.AppImage" ]] || {
  printf 'ERROR: the portable build did not produce the %s .deb, .rpm and AppImage.\n' "$EXPECTED_TAG" >&2
  exit 1
}

printf '\n==> Building the Flatpak bundle from the portable .deb\n'
FLATPAK_DIR="$(mktemp -d -t ostudy-release-flatpak.XXXXXXXX)"
trap 'rm -rf -- "$FLATPAK_DIR"' EXIT
"$SCRIPT_DIR/build-flatpak.sh" --container --deb "$DEB" --output "$FLATPAK_DIR"
cp "$FLATPAK_DIR/ostudy-$EXPECTED_TAG-linux-$ARCH.flatpak" "$OUT_DIR/"

git archive --format=tar.gz --prefix="ostudy-$EXPECTED_TAG/" \
  --output="$OUT_DIR/ostudy-$EXPECTED_TAG-source.tar.gz" HEAD

{
  printf 'tag=%s\n' "$EXPECTED_TAG"
  printf 'release_commit=%s\n' "$(git rev-parse HEAD)"
  printf 'pdfium_build=%s\n' "$PDFIUM_BUILD"
} >> "$OUT_DIR/BUILD-INFO.txt"
(
  cd "$OUT_DIR"
  sha256sum -- *.deb *.rpm *.AppImage *.flatpak *.tar.gz BUILD-INFO.txt > SHA256SUMS.txt
)

printf '\nRelease artifacts created in: %s\n' "$OUT_DIR"

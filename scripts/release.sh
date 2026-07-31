#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/cargo-target.sh"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(nproc)}"
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
prepare_cargo_target "$ROOT_DIR"
pnpm install --frozen-lockfile

printf '\n==> Building Linux Debian and AppImage bundles\n'
node "$SCRIPT_DIR/with-immutable-cargo-lock.mjs" pnpm tauri build --bundles deb,appimage --ci

case "$(uname -m)" in
  x86_64) ARCH="x86_64" ;;
  aarch64|arm64) ARCH="aarch64" ;;
  *) printf 'ERROR: unsupported release architecture: %s\n' "$(uname -m)" >&2; exit 1 ;;
esac

OUT_DIR="$ROOT_DIR/artifacts/$EXPECTED_TAG/linux-$ARCH"
rm -rf -- "$ROOT_DIR/artifacts/$EXPECTED_TAG"
mkdir -p "$OUT_DIR"
shopt -s nullglob
DEB_FILES=(src-tauri/target/release/bundle/deb/*.deb)
APPIMAGE_FILES=(src-tauri/target/release/bundle/appimage/*.AppImage)
(( ${#DEB_FILES[@]} == 1 )) || { printf 'ERROR: expected exactly one Debian bundle.\n' >&2; exit 1; }
(( ${#APPIMAGE_FILES[@]} == 1 )) || { printf 'ERROR: expected exactly one AppImage bundle.\n' >&2; exit 1; }

cp "${DEB_FILES[0]}" "$OUT_DIR/ostudy-$EXPECTED_TAG-linux-$ARCH.deb"
cp "${APPIMAGE_FILES[0]}" "$OUT_DIR/ostudy-$EXPECTED_TAG-linux-$ARCH.AppImage"
chmod +x "$OUT_DIR/ostudy-$EXPECTED_TAG-linux-$ARCH.AppImage"
git archive --format=tar.gz --prefix="ostudy-$EXPECTED_TAG/" \
  --output="$OUT_DIR/ostudy-$EXPECTED_TAG-source.tar.gz" HEAD

{
  printf 'version=%s\n' "$VERSION"
  printf 'tag=%s\n' "$EXPECTED_TAG"
  printf 'commit=%s\n' "$(git rev-parse HEAD)"
  printf 'architecture=%s\n' "$ARCH"
  printf 'built_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'cargo_jobs=%s\n' "$CARGO_BUILD_JOBS"
} > "$OUT_DIR/BUILD-INFO.txt"
(
  cd "$OUT_DIR"
  sha256sum BUILD-INFO.txt *.deb *.AppImage *.tar.gz > SHA256SUMS.txt
)

printf '\nRelease artifacts created in: %s\n' "$OUT_DIR"

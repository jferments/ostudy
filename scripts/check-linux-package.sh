#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
source "$ROOT_DIR/versions.env"
source "$SCRIPT_DIR/lib/node-toolchain.bash"
ostudy_activate_node_toolchain
source "$SCRIPT_DIR/cargo-target.sh"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(nproc)}"
cd "$ROOT_DIR"

OUTPUT_DIR="$ROOT_DIR/.artifacts/linux-package"
while (($#)); do
  case "$1" in
    --output)
      [[ $# -ge 2 ]] || { printf 'ERROR: --output requires a directory.\n' >&2; exit 2; }
      OUTPUT_DIR="$2"; shift 2 ;;
    -h|--help)
      printf '%s\n' 'Usage: ./scripts/check-linux-package.sh [--output DIRECTORY]'
      printf '%s\n' 'Build and verify the production Debian package for the exact current source.'
      exit 0 ;;
    *) printf 'ERROR: unknown argument: %s\n' "$1" >&2; exit 2 ;;
  esac
done

git rev-parse --is-inside-work-tree >/dev/null
git diff --quiet && git diff --cached --quiet || {
  printf 'ERROR: production package qualification requires a clean tracked worktree.\n' >&2
  exit 1
}
node "$SCRIPT_DIR/verify-linux-package-data-safety.mjs"
prepare_cargo_target "$ROOT_DIR"
pnpm install --frozen-lockfile
"$SCRIPT_DIR/install-native-deps.sh" --pdfium-only

BUNDLE_ROOT="$CARGO_TARGET_DIR/release/bundle"
rm -rf -- "$BUNDLE_ROOT/deb"
printf '\n==> Building production Debian package\n'
pnpm tauri build --config src-tauri/tauri.linux.conf.json --bundles deb --ci -- --locked

shopt -s nullglob
DEB_FILES=("$BUNDLE_ROOT"/deb/*.deb)
(( ${#DEB_FILES[@]} == 1 )) || {
  printf 'ERROR: expected exactly one Debian bundle under %s/deb.\n' "$BUNDLE_ROOT" >&2
  exit 1
}
"$SCRIPT_DIR/verify-deb-package.sh" "${DEB_FILES[0]}" "$CARGO_TARGET_DIR/release/ostudy"

case "$(uname -m)" in
  x86_64) ARCH="x86_64" ;;
  aarch64|arm64) ARCH="aarch64" ;;
  *) printf 'ERROR: unsupported package architecture: %s\n' "$(uname -m)" >&2; exit 1 ;;
esac
VERSION="$(node -p "require('./package.json').version")"
SOURCE_SHA="$(git rev-parse HEAD)"
rm -rf -- "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"
DESTINATION="$OUTPUT_DIR/ostudy-v$VERSION-linux-$ARCH.deb"
cp -- "${DEB_FILES[0]}" "$DESTINATION"
{
  printf 'version=%s\n' "$VERSION"
  printf 'commit=%s\n' "$SOURCE_SHA"
  printf 'architecture=%s\n' "$ARCH"
  printf 'application_identifier=io.github.jferments.ostudy\n'
  printf 'package=%s\n' "$(basename -- "$DESTINATION")"
  printf 'built_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'cargo_target_dir=%s\n' "$CARGO_TARGET_DIR"
  printf 'pdfium_build=%s\n' "$PDFIUM_BUILD"
} > "$OUTPUT_DIR/BUILD-INFO.txt"
(
  cd "$OUTPUT_DIR"
  sha256sum "$(basename -- "$DESTINATION")" BUILD-INFO.txt > SHA256SUMS.txt
)
printf '\nProduction Debian package qualified for %s\n' "$SOURCE_SHA"
printf 'Package: %s\n' "$DESTINATION"
printf 'Checksums: %s\n' "$OUTPUT_DIR/SHA256SUMS.txt"

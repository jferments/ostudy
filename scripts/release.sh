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

usage() {
  cat <<'USAGE'
Usage: ./scripts/release.sh [TAG] [--gate-only | --skip-gate] [--linux-only]

Qualify and package a tagged release. With no option it runs the complete
gate in a clean source copy, then builds the Linux files and the signed
Android APK.

--gate-only   only the gate. The tag's Release gate workflow runs this on the
              CI runner, which deliberately has no Docker.
--skip-gate   only the files, on a maintainer machine with Podman or Docker
              and flatpak-builder. HEAD must be the tag, and the tag's Release
              gate run must have passed (checked with gh).
--linux-only  leave out the Android APK.

The APK is the universal release build, signed by
scripts/sign-android-release.sh with the release key named in
OSTUDY_ANDROID_KEYSTORE (password in OSTUDY_ANDROID_KEYSTORE_PASSWORD).

The files go to artifacts/TAG/release; publish them with
./scripts/publish-github-release.sh and ostudy-io's tools/publish-release.sh.
USAGE
}

MODE=all
ANDROID=1
REQUESTED_TAG=""
while (($#)); do
  case "$1" in
    --gate-only) MODE=gate; shift ;;
    --skip-gate) MODE=packages; shift ;;
    --linux-only) ANDROID=0; shift ;;
    -h|--help) usage; exit 0 ;;
    -*) printf 'ERROR: unknown option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
    *) [[ -z "$REQUESTED_TAG" ]] || { usage >&2; exit 2; }; REQUESTED_TAG="$1"; shift ;;
  esac
done

VERSION="$(node -p "require('./package.json').version")"
EXPECTED_TAG="v$VERSION"
REQUESTED_TAG="${REQUESTED_TAG:-$EXPECTED_TAG}"
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

if [[ "$MODE" == packages ]]; then
  tag_commit="$(git rev-parse --verify --quiet "refs/tags/$EXPECTED_TAG^{commit}")" || {
    printf 'ERROR: there is no tag %s here; fetch tags first.\n' "$EXPECTED_TAG" >&2
    exit 1
  }
  [[ "$(git rev-parse HEAD)" == "$tag_commit" ]] || {
    printf 'ERROR: HEAD is not %s; run git checkout --detach %s first.\n' "$EXPECTED_TAG" "$EXPECTED_TAG" >&2
    exit 1
  }
  command -v gh >/dev/null 2>&1 || { printf 'ERROR: --skip-gate checks the Release gate run with gh; install and log in to gh.\n' >&2; exit 1; }
  gate="$(gh run list --repo "${OSTUDY_REPO:-jferments/ostudy}" --workflow release.yml --commit "$tag_commit" \
    --json conclusion --jq 'map(select(.conclusion == "success")) | length')"
  [[ "$gate" -ge 1 ]] || {
    printf 'ERROR: the Release gate for %s (%s) has not passed yet.\n' "$EXPECTED_TAG" "${tag_commit:0:12}" >&2
    exit 1
  }
else
  "$SCRIPT_DIR/release-check.sh"
fi

if [[ "$MODE" == gate ]]; then
  mkdir -p "$ROOT_DIR/artifacts/$EXPECTED_TAG"
  {
    printf 'tag=%s\n' "$EXPECTED_TAG"
    printf 'release_commit=%s\n' "$(git rev-parse HEAD)"
    printf 'gate=passed\n'
  } > "$ROOT_DIR/artifacts/$EXPECTED_TAG/GATE.txt"
  printf '\nRelease gate passed for %s.\n' "$EXPECTED_TAG"
  exit 0
fi

if [[ "$ANDROID" == 1 ]]; then
  [[ -n "${OSTUDY_ANDROID_KEYSTORE:-}" && -n "${OSTUDY_ANDROID_KEYSTORE_PASSWORD:-}" ]] || {
    printf 'ERROR: set OSTUDY_ANDROID_KEYSTORE and OSTUDY_ANDROID_KEYSTORE_PASSWORD for the Android APK, or pass --linux-only.\n' >&2
    exit 1
  }
fi

case "$(uname -m)" in
  x86_64) ARCH="x86_64" ;;
  *) printf 'ERROR: Linux releases are built on x86_64 only, not %s.\n' "$(uname -m)" >&2; exit 1 ;;
esac

OUT_DIR="$ROOT_DIR/artifacts/$EXPECTED_TAG/release"
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
"$SCRIPT_DIR/build-flatpak.sh" --deb "$DEB" --output "$FLATPAK_DIR"
cp "$FLATPAK_DIR/ostudy-$EXPECTED_TAG-linux-$ARCH.flatpak" "$OUT_DIR/"

if [[ "$ANDROID" == 1 ]]; then
  # The universal release APK (every ABI) installs from a browser download;
  # Gradle leaves it unsigned and the release key never enters the build.
  printf '\n==> Building and signing the Android APK\n'
  "$SCRIPT_DIR/android-build.sh" --release --apk --target all
  UNSIGNED_APK="$ROOT_DIR/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk"
  APK="$OUT_DIR/ostudy-$EXPECTED_TAG-android.apk"
  "$SCRIPT_DIR/sign-android-release.sh" "$UNSIGNED_APK" "$APK"
  "$SCRIPT_DIR/verify-android-package.sh" "$APK" release
fi

git archive --format=tar.gz --prefix="ostudy-$EXPECTED_TAG/" \
  --output="$OUT_DIR/ostudy-$EXPECTED_TAG-source.tar.gz" HEAD

{
  printf 'tag=%s\n' "$EXPECTED_TAG"
  printf 'release_commit=%s\n' "$(git rev-parse HEAD)"
  printf 'pdfium_build=%s\n' "$PDFIUM_BUILD"
  if [[ "$ANDROID" == 1 ]]; then
    printf 'android_certificate_sha256=%s\n' "$(tr -d ' :\n' < "$ROOT_DIR/src-tauri/packaging/android/release-certificate.sha256")"
  fi
} >> "$OUT_DIR/BUILD-INFO.txt"
(
  cd "$OUT_DIR"
  shopt -s nullglob
  sha256sum -- *.deb *.rpm *.AppImage *.flatpak *.apk *.tar.gz BUILD-INFO.txt > SHA256SUMS.txt
)

printf '\nRelease artifacts created in: %s\n' "$OUT_DIR"

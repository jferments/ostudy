#!/usr/bin/env bash
set -euo pipefail

# Prepare the exact Pdfium runtime that matches pdfium-render's pinned API.
#
# The binary is a build/runtime dependency of ostudy, not user-managed AI
# infrastructure. It is downloaded only when missing, cached outside Git, and
# verified against GitHub's digest for the immutable chromium/7881 release.

OSTUDY_PDFIUM_BUILD="7881"
OSTUDY_PDFIUM_RELEASE_TAG="chromium/7881"
OSTUDY_PDFIUM_RELEASE_API="https://api.github.com/repos/bblanchon/pdfium-binaries/releases/tags/chromium%2F7881"

ostudy_pdfium_platform() {
  if [[ -n "${OSTUDY_PDFIUM_TARGET:-}" ]]; then
    printf '%s\n' "$OSTUDY_PDFIUM_TARGET"
    return
  fi

  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os:$arch" in
    Linux:x86_64|Linux:amd64) printf '%s\n' "linux-x64" ;;
    Linux:aarch64|Linux:arm64) printf '%s\n' "linux-arm64" ;;
    Darwin:x86_64|Darwin:amd64) printf '%s\n' "mac-x64" ;;
    Darwin:arm64|Darwin:aarch64) printf '%s\n' "mac-arm64" ;;
    MINGW*:x86_64|MSYS*:x86_64|CYGWIN*:x86_64) printf '%s\n' "win-x64" ;;
    MINGW*:arm64|MSYS*:arm64|CYGWIN*:arm64) printf '%s\n' "win-arm64" ;;
    *)
      echo "Unsupported Pdfium host platform: $os $arch" >&2
      echo "Set OSTUDY_PDFIUM_TARGET to a supported bblanchon target if cross-compiling." >&2
      return 1
      ;;
  esac
}

ostudy_pdfium_library_name() {
  local platform="$1"
  case "$platform" in
    linux-*) printf '%s\n' "libpdfium.so" ;;
    mac-*) printf '%s\n' "libpdfium.dylib" ;;
    win-*) printf '%s\n' "pdfium.dll" ;;
    *) return 1 ;;
  esac
}

ostudy_prepare_pdfium() {
  local root platform asset library_name cache_dir archive extract_dir resource_dir library_path
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  platform="$(ostudy_pdfium_platform)"
  asset="pdfium-${platform}.tgz"
  library_name="$(ostudy_pdfium_library_name "$platform")"
  cache_dir="${OSTUDY_PDFIUM_CACHE_DIR:-$HOME/.cache/ostudy/pdfium/${OSTUDY_PDFIUM_BUILD}/${platform}}"
  archive="$cache_dir/$asset"
  extract_dir="$cache_dir/extracted"
  resource_dir="$root/src-tauri/resources/pdfium"
  library_path="$resource_dir/$library_name"

  mkdir -p "$cache_dir" "$resource_dir"

  if [[ ! -f "$archive" || ! -f "$extract_dir/.verified" ]]; then
    local release_json asset_url digest expected actual tmp
    release_json="$(curl --fail --silent --show-error --location \
      --retry 3 --retry-delay 1 \
      -H 'Accept: application/vnd.github+json' \
      "$OSTUDY_PDFIUM_RELEASE_API")"
    asset_url="$(printf '%s' "$release_json" | jq -r --arg name "$asset" \
      '.assets[] | select(.name == $name) | .browser_download_url' | head -n1)"
    digest="$(printf '%s' "$release_json" | jq -r --arg name "$asset" \
      '.assets[] | select(.name == $name) | .digest' | head -n1)"
    if [[ -z "$asset_url" || "$asset_url" == "null" ]]; then
      echo "Pdfium release $OSTUDY_PDFIUM_RELEASE_TAG does not contain $asset" >&2
      return 1
    fi
    if [[ ! "$digest" =~ ^sha256:[0-9a-f]{64}$ ]]; then
      echo "Pdfium release metadata did not provide a SHA-256 digest for $asset" >&2
      return 1
    fi
    expected="${digest#sha256:}"

    if [[ -f "$archive" ]]; then
      actual="$(sha256sum "$archive" | awk '{print $1}')"
      if [[ "$actual" != "$expected" ]]; then
        rm -f "$archive"
      fi
    fi

    if [[ ! -f "$archive" ]]; then
      tmp="$archive.partial"
      rm -f "$tmp"
      curl --fail --silent --show-error --location \
        --retry 3 --retry-delay 1 \
        "$asset_url" -o "$tmp"
      actual="$(sha256sum "$tmp" | awk '{print $1}')"
      if [[ "$actual" != "$expected" ]]; then
        rm -f "$tmp"
        echo "Pdfium SHA-256 mismatch for $asset" >&2
        return 1
      fi
      mv "$tmp" "$archive"
    fi

    rm -rf "$extract_dir"
    mkdir -p "$extract_dir"
    tar -xzf "$archive" -C "$extract_dir"
    printf '%s  %s\n' "$expected" "$asset" > "$extract_dir/.verified"
  fi

  local found
  found="$(find "$extract_dir" -type f -name "$library_name" -print -quit)"
  if [[ -z "$found" ]]; then
    echo "Verified Pdfium archive did not contain $library_name" >&2
    return 1
  fi
  cp "$found" "$library_path"
  chmod u+rw "$library_path"
  export OSTUDY_PDFIUM_LIBRARY="$library_path"
  export OSTUDY_PDFIUM_BUILD
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  ostudy_prepare_pdfium
  printf '%s\n' "$OSTUDY_PDFIUM_LIBRARY"
fi

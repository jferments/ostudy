#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
MANIFEST="$SCRIPT_DIR/native-deps-manifest.json"
PDFIUM_DEST="$ROOT_DIR/src-tauri/resources/native/pdfium"
INSTALL_TESSERACT=1
TARGET=""

usage() {
  cat <<'EOF'
Usage: ./scripts/install-native-deps.sh [options]

Options:
  --pdfium-only          Provision/verify PDFium only (used by build/check CI).
  --with-tesseract       Also ensure desktop Tesseract + English data are installed.
  --target TARGET        Stage a specific manifest target (for example linux-x64,
                         macos-arm64, android-arm64, ios-device-arm64).
  --help                 Show this help.

PDFium is downloaded only from the immutable, pinned release in
scripts/native-deps-manifest.json and is SHA-256 verified before extraction.
Set OSTUDY_PDFIUM_PATH to use an explicitly managed/system library instead.
EOF
}

while (( $# > 0 )); do
  case "$1" in
    --pdfium-only) INSTALL_TESSERACT=0 ;;
    --with-tesseract) INSTALL_TESSERACT=1 ;;
    --target)
      shift
      [[ $# -gt 0 ]] || { printf 'ERROR: --target requires a value.\n' >&2; exit 2; }
      TARGET="$1"
      ;;
    --help|-h) usage; exit 0 ;;
    *) printf 'ERROR: unknown option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

[[ -f "$MANIFEST" ]] || { printf 'ERROR: native dependency manifest is missing.\n' >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { printf 'ERROR: python3 is required to read the audited native manifest.\n' >&2; exit 1; }
command -v curl >/dev/null 2>&1 || { printf 'ERROR: curl is required to download pinned PDFium artifacts.\n' >&2; exit 1; }
command -v tar >/dev/null 2>&1 || { printf 'ERROR: tar is required to extract PDFium artifacts.\n' >&2; exit 1; }

host_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os:$arch" in
    Linux:x86_64|Linux:amd64) printf 'linux-x64\n' ;;
    Linux:aarch64|Linux:arm64) printf 'linux-arm64\n' ;;
    Darwin:x86_64|Darwin:amd64) printf 'macos-x64\n' ;;
    Darwin:arm64|Darwin:aarch64) printf 'macos-arm64\n' ;;
    *)
      printf 'ERROR: automatic PDFium target detection does not support %s/%s. Use --target with a manifest target.\n' "$os" "$arch" >&2
      return 1
      ;;
  esac
}

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    printf 'ERROR: sha256sum or shasum is required for native artifact verification.\n' >&2
    return 1
  fi
}

TARGET="${TARGET:-$(host_target)}"
IFS=$'\t' read -r PDFIUM_BUILD BASE_URL ARCHIVE EXPECTED_SHA LIBRARY_PATH < <(
  python3 - "$MANIFEST" "$TARGET" <<'PY'
import json, pathlib, sys
manifest = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
target = sys.argv[2]
pdfium = manifest["pdfium"]
try:
    artifact = pdfium["artifacts"][target]
except KeyError:
    raise SystemExit(f"ERROR: no pinned PDFium artifact is defined for target {target!r}.")
print(
    pdfium["apiBuild"],
    pdfium["binarySource"]["releaseBaseUrl"],
    artifact["archive"],
    artifact["sha256"],
    artifact["library"],
    sep="\t",
)
PY
)

library_name="$(basename -- "$LIBRARY_PATH")"
installed_library="$PDFIUM_DEST/$library_name"
provenance="$PDFIUM_DEST/PROVENANCE.env"

if [[ -n "${OSTUDY_PDFIUM_PATH:-}" ]]; then
  explicit="$OSTUDY_PDFIUM_PATH"
  if [[ -d "$explicit" ]]; then
    explicit="$explicit/$library_name"
  fi
  [[ -f "$explicit" ]] || {
    printf 'ERROR: OSTUDY_PDFIUM_PATH does not resolve to a PDFium library: %s\n' "$explicit" >&2
    exit 1
  }
  printf 'Using operator-managed PDFium: %s\n' "$explicit"
else
  reuse=0
  if [[ -f "$installed_library" && -f "$provenance" ]] \
    && grep -Fqx "target=$TARGET" "$provenance" \
    && grep -Fqx "archive_sha256=$EXPECTED_SHA" "$provenance"; then
    installed_sha="$(hash_file "$installed_library")"
    if grep -Fqx "library_sha256=$installed_sha" "$provenance"; then
      reuse=1
      printf 'OK   PDFium %s for %s is already staged and verified.\n' "$PDFIUM_BUILD" "$TARGET"
    fi
  fi

  if (( reuse == 0 )); then
    temp_root="$(mktemp -d -t ostudy-pdfium.XXXXXXXX)"
    cleanup() { rm -rf -- "$temp_root"; }
    trap cleanup EXIT
    archive_path="$temp_root/$ARCHIVE"
    extract_dir="$temp_root/extract"
    mkdir -p "$extract_dir"

    printf 'Downloading pinned PDFium %s for %s...\n' "$PDFIUM_BUILD" "$TARGET"
    curl --fail --location --proto '=https' --tlsv1.2 \
      "$BASE_URL/$ARCHIVE" \
      --output "$archive_path"
    actual_sha="$(hash_file "$archive_path")"
    [[ "$actual_sha" == "$EXPECTED_SHA" ]] || {
      printf 'ERROR: PDFium archive SHA-256 mismatch. Expected %s, got %s.\n' "$EXPECTED_SHA" "$actual_sha" >&2
      exit 1
    }

    tar -xzf "$archive_path" -C "$extract_dir"
    [[ -f "$extract_dir/$LIBRARY_PATH" ]] || {
      printf 'ERROR: verified archive does not contain expected library path %s.\n' "$LIBRARY_PATH" >&2
      exit 1
    }

    rm -rf -- "$PDFIUM_DEST"
    mkdir -p "$PDFIUM_DEST"
    cp "$extract_dir/$LIBRARY_PATH" "$installed_library"
    [[ -f "$extract_dir/LICENSE" ]] && cp "$extract_dir/LICENSE" "$PDFIUM_DEST/PDFIUM-LICENSE.txt"
    library_sha="$(hash_file "$installed_library")"
    cat > "$provenance" <<EOF
pdfium_build=$PDFIUM_BUILD
target=$TARGET
archive=$ARCHIVE
archive_sha256=$EXPECTED_SHA
library=$library_name
library_sha256=$library_sha
source=$BASE_URL/$ARCHIVE
EOF
    printf 'OK   PDFium %s staged at %s\n' "$PDFIUM_BUILD" "$installed_library"
  fi
fi

ensure_tesseract() {
  if command -v tesseract >/dev/null 2>&1 \
    && tesseract --list-langs 2>/dev/null | grep -Fqx 'eng'; then
    printf 'OK   Tesseract with English trained data is available.\n'
    return 0
  fi

  case "$(uname -s)" in
    Linux)
      if command -v apt-get >/dev/null 2>&1; then
        printf 'Installing Tesseract through apt...\n'
        if [[ "$(id -u)" == "0" ]]; then
          apt-get update
          apt-get install -y tesseract-ocr tesseract-ocr-eng
        elif command -v sudo >/dev/null 2>&1; then
          sudo apt-get update
          sudo apt-get install -y tesseract-ocr tesseract-ocr-eng
        else
          printf 'ERROR: apt requires root/sudo to install Tesseract.\n' >&2
          return 1
        fi
      else
        printf 'ERROR: automatic Tesseract installation is currently implemented for Debian/Ubuntu Linux. Install Tesseract 5 with English trained data using your distribution package manager.\n' >&2
        return 1
      fi
      ;;
    Darwin)
      if command -v brew >/dev/null 2>&1; then
        brew install tesseract
      elif command -v port >/dev/null 2>&1; then
        sudo port install tesseract
      else
        printf 'ERROR: install Homebrew or MacPorts, then install Tesseract, or build Tesseract from its official source.\n' >&2
        return 1
      fi
      ;;
    *)
      printf 'ERROR: use scripts/install-native-deps.ps1 on Windows.\n' >&2
      return 1
      ;;
  esac

  command -v tesseract >/dev/null 2>&1 \
    && tesseract --list-langs 2>/dev/null | grep -Fqx 'eng' || {
      printf 'ERROR: Tesseract installation completed but English trained data is unavailable.\n' >&2
      return 1
    }
  printf 'OK   Tesseract with English trained data is available.\n'
}

if (( INSTALL_TESSERACT == 1 )); then
  ensure_tesseract
fi

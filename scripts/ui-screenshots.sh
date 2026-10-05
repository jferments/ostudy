#!/usr/bin/env bash
# Capture the main oStudy screens for before/after UI review, and compare two
# captures. See docs/development/ui-screenshots.md.
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
ENGINE="$ROOT_DIR/tests/webdriver/ui_screenshots.py"
CACHE_DIR="${OSTUDY_UI_SCREENSHOTS_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/ostudy/ui-screenshots}"
# Application data lives at one fixed path so paths shown on screen (the
# Diagnostics log location) are the same in every capture.
RUN_HOME="${OSTUDY_UI_SCREENSHOTS_HOME:-/tmp/ostudy-ui-screenshots}"
KEEP_BINARIES="${OSTUDY_UI_SCREENSHOTS_KEEP:-3}"
CAPTURE_TIMEOUT="${OSTUDY_UI_SCREENSHOTS_TIMEOUT:-2400}"

usage() {
  cat >&2 <<'USAGE'
Usage:
  ./scripts/ui-screenshots.sh capture --output DIR [--ref REV | --application PATH]
                                      [--screens LIST] [--themes LIST] [--widths LIST]
  ./scripts/ui-screenshots.sh compare BEFORE_DIR AFTER_DIR --output DIR
                                      [--screens LIST] [--themes LIST] [--widths LIST]
  ./scripts/ui-screenshots.sh build REV
  ./scripts/ui-screenshots.sh list

capture   Seed a disposable library and photograph every selected screen.
          Without --ref or --application it uses this checkout's staged build
          (./scripts/build.sh). --ref builds REV once in a cached worktree and
          reuses the binary by commit SHA.
compare   Write a self-contained index.html and image copies to DIR.
build     Build and cache the binary for REV without capturing.
list      Print the screen ids.

LIST is comma-separated: screens are ids from `list` (or `settings`,
`practice`); themes are light, dark, console, synthwave; widths are 390,
1024, 1440 or phone, tablet, desktop.
USAGE
}

die() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

log() {
  printf '[ui-screenshots] %s\n' "$*" >&2
}

require_commands() {
  local name
  for name in "$@"; do
    command -v "$name" >/dev/null 2>&1 || die "$name is required. Re-run ./scripts/setup-ubuntu.sh."
  done
}

# WebP output needs libwebp's cwebp or ImageMagick; the comparison page needs ImageMagick.
require_image_tools() {
  local need="$1"
  if command -v magick >/dev/null 2>&1 || command -v convert >/dev/null 2>&1; then
    return 0
  fi
  if [[ "$need" == encode ]] && command -v cwebp >/dev/null 2>&1; then
    return 0
  fi
  die "ImageMagick is required (for example: sudo apt-get install imagemagick)."
}

# --- cached builds by commit -------------------------------------------------

cached_binary_path() {
  printf '%s/bin/ostudy-%s\n' "$CACHE_DIR" "$1"
}

prune_cached_binaries() {
  local keep="$1"
  local stale
  # Newest first; keep the most recently used binaries.
  while IFS= read -r stale; do
    rm -f -- "$stale"
  done < <(find "$CACHE_DIR/bin" -maxdepth 1 -type f -name 'ostudy-*' -printf '%T@ %p\n' 2>/dev/null |
    sort -rn | awk -v keep="$keep" 'NR > keep { print $2 }')
}

# Executables in the shared Cargo target that were built from SOURCE_DIR,
# newest first. Other checkouts may share the target and replace
# target/debug/ostudy at any moment, so match on the source path that each
# binary embeds instead of trusting the uplifted copy.
worktree_executables() {
  local target_dir="$1" source_dir="$2" pattern="$3"
  local file
  while IFS= read -r file; do
    if grep -qF -- "$source_dir/src-tauri" "$file"; then
      printf '%s\n' "$file"
    fi
  done < <(find "$target_dir/debug/deps" -maxdepth 1 -type f -perm -u+x -regextype posix-extended \
    -regex ".*/(${pattern})-[0-9a-f]{16}" -printf '%T@ %p\n' 2>/dev/null | sort -rn | cut -d' ' -f2-)
}

# The cached copy is all a capture needs. Drop the cache worktree's own
# executables (and their uplifted hard links) so a before-build does not leave
# gigabytes in the shared target; compiled dependencies stay for the next build.
remove_worktree_executables() {
  local target_dir="$1" source_dir="$2"
  local file link
  while IFS= read -r file; do
    while IFS= read -r link; do
      rm -f -- "$link"
    done < <(find "$target_dir/debug" -maxdepth 1 -type f -samefile "$file" 2>/dev/null)
    rm -f -- "$file"
  done < <(worktree_executables "$target_dir" "$source_dir" 'ostudy|export_[a-z_]+')
}

build_commit() {
  local sha="$1"
  local binary
  binary="$(cached_binary_path "$sha")"
  if [[ -x "$binary" ]]; then
    touch -- "$binary"
    log "Using the cached build of ${sha:0:12}: $binary"
    printf '%s\n' "$binary"
    return 0
  fi

  require_commands git pnpm cargo
  # shellcheck disable=SC1091
  source "$SCRIPT_DIR/cargo-target.sh"
  local target_dir="${CARGO_TARGET_DIR:-$(ostudy_default_cargo_target_dir "$ROOT_DIR")}"
  [[ "$target_dir" == /* ]] || target_dir="$ROOT_DIR/$target_dir"
  local pdfium="$ROOT_DIR/src-tauri/resources/native/pdfium"
  [[ -d "$pdfium" ]] || die "PDFium is not provisioned in this checkout. Run ./scripts/install-native-deps.sh --pdfium-only."

  # One reusable worktree keeps Cargo's unit hashes stable between builds, so
  # each build replaces the previous one's outputs instead of adding more.
  local source_dir="$CACHE_DIR/source"
  mkdir -p "$CACHE_DIR/bin"
  if git -C "$source_dir" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    git -C "$source_dir" checkout --quiet --detach --force "$sha"
    git -C "$source_dir" clean -fdxq -e node_modules -e src-tauri/resources/native/pdfium
  else
    rm -rf -- "$source_dir"
    git -C "$ROOT_DIR" worktree prune
    git -C "$ROOT_DIR" worktree add --quiet --detach "$source_dir" "$sha"
  fi
  mkdir -p "$source_dir/src-tauri/resources/native"
  rm -rf -- "$source_dir/src-tauri/resources/native/pdfium"
  cp -a -- "$pdfium" "$source_dir/src-tauri/resources/native/pdfium"

  log "Building ${sha:0:12} with the embedded frontend (Cargo target: $target_dir)"
  local started=$SECONDS
  (
    cd "$source_dir"
    pnpm install --frozen-lockfile --prefer-offline >&2
    CARGO_TARGET_DIR="$target_dir" \
      pnpm tauri build --config src-tauri/tauri.dev.conf.json --debug --no-bundle --ci -- --locked >&2
  )
  local built
  built="$(worktree_executables "$target_dir" "$source_dir" ostudy)"
  built="${built%%$'\n'*}"
  [[ -n "$built" ]] || die "Could not find the binary built from $source_dir in $target_dir."
  local temporary="$binary.$$.tmp"
  cp -- "$built" "$temporary"
  remove_worktree_executables "$target_dir" "$source_dir"
  # Debug information is three quarters of the file and screenshots never need it.
  if command -v strip >/dev/null 2>&1; then
    strip --strip-debug "$temporary"
  fi
  chmod 0755 "$temporary"
  mv -f -- "$temporary" "$binary"
  log "Built ${sha:0:12} in $((SECONDS - started))s"
  prune_cached_binaries "$KEEP_BINARIES"
  printf '%s\n' "$binary"
}

# Pin the captures to US English whatever the host locale is. When the host
# lacks en_US.UTF-8, compile it once into the cache instead of changing the
# system.
capture_locale_env() {
  if locale -a 2>/dev/null | grep -qixE 'en_US\.utf-?8'; then
    printf '%s\n' LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8
    return 0
  fi
  local directory="$CACHE_DIR/locale"
  if [[ ! -f "$directory/en_US.UTF-8/LC_CTYPE" ]] && command -v localedef >/dev/null 2>&1; then
    mkdir -p "$directory"
    localedef -i en_US -f UTF-8 "$directory/en_US.UTF-8" >/dev/null 2>&1 || true
  fi
  if [[ -f "$directory/en_US.UTF-8/LC_CTYPE" ]]; then
    printf '%s\n' "LOCPATH=$directory" LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8
    return 0
  fi
  log "en_US.UTF-8 is unavailable; dates and numbers use the C locale."
  printf '%s\n' LANG=C.UTF-8 LC_ALL=C.UTF-8
}

resolve_commit() {
  git -C "$ROOT_DIR" rev-parse --verify --quiet "$1^{commit}" || die "Unknown revision: $1"
}

source_json() {
  # source_json SHA REF DIRTY
  python3 - "$1" "$2" "$3" "$ROOT_DIR" <<'PY'
import json, subprocess, sys
sha, ref, dirty, root = sys.argv[1:5]
subject = ""
if sha:
    subject = subprocess.run(["git", "-C", root, "log", "-1", "--format=%s", sha],
                             capture_output=True, text=True).stdout.strip()
print(json.dumps({"sha": sha, "ref": ref, "dirty": dirty == "1", "subject": subject}))
PY
}

# --- commands ----------------------------------------------------------------

COMMAND="${1:-}"
[[ $# -gt 0 ]] && shift
case "$COMMAND" in
  list)
    exec python3 "$ENGINE" list
    ;;
  compare)
    require_image_tools compare
    exec python3 "$ENGINE" compare "$@"
    ;;
  build)
    [[ $# -eq 1 ]] || { usage; exit 2; }
    mkdir -p "$(dirname -- "$CACHE_DIR")"
    exec 9>"$CACHE_DIR.lock"
    flock 9
    build_commit "$(resolve_commit "$1")" >/dev/null
    exit 0
    ;;
  capture) ;;
  -h | --help | help)
    usage
    exit 0
    ;;
  *)
    usage
    exit 2
    ;;
esac

REF=""
APPLICATION=""
OUTPUT=""
FILTERS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --ref | --application | --output | --screens | --themes | --widths)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      case "$1" in
        --ref) REF="$2" ;;
        --application) APPLICATION="$2" ;;
        --output) OUTPUT="$2" ;;
        *) FILTERS+=("$1" "$2") ;;
      esac
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done
[[ -n "$OUTPUT" ]] || { usage; exit 2; }
[[ -z "$REF" || -z "$APPLICATION" ]] || die "Use either --ref or --application, not both."

export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
require_commands python3 tauri-driver WebKitWebDriver xvfb-run timeout flock
require_image_tools encode
mkdir -p "$(dirname -- "$CACHE_DIR")" "$(dirname -- "$RUN_HOME")"

if [[ -n "$APPLICATION" ]]; then
  [[ -x "$APPLICATION" ]] || die "Not an executable: $APPLICATION"
  APPLICATION="$(cd -- "$(dirname -- "$APPLICATION")" && pwd)/$(basename -- "$APPLICATION")"
  SOURCE="$(source_json "" "application" 0)"
elif [[ -n "$REF" ]]; then
  SHA="$(resolve_commit "$REF")"
  exec 9>"$CACHE_DIR.lock"
  flock 9
  APPLICATION="$(build_commit "$SHA")"
  exec 9>&-
  SOURCE="$(source_json "$SHA" "$REF" 0)"
else
  export OSTUDY_ROOT_DIR="$ROOT_DIR"
  # shellcheck disable=SC1091
  source "$SCRIPT_DIR/build-state.sh"
  ostudy_require_current_build
  APPLICATION="$OSTUDY_BINARY"
  DIRTY=0
  [[ -z "$(git -C "$ROOT_DIR" status --porcelain)" ]] || DIRTY=1
  SOURCE="$(source_json "$(git -C "$ROOT_DIR" rev-parse HEAD)" "working tree" "$DIRTY")"
fi

PDFIUM_DIR="$ROOT_DIR/src-tauri/resources/native/pdfium"
[[ -d "$PDFIUM_DIR" ]] || die "PDFium is not provisioned in this checkout. Run ./scripts/install-native-deps.sh --pdfium-only."

# The run directory is deleted before and after each capture, so only accept a
# dedicated path.
[[ "$RUN_HOME" == /* && "$(basename -- "$RUN_HOME")" == *ostudy-ui-screenshots* ]] ||
  die "OSTUDY_UI_SCREENSHOTS_HOME must be an absolute path whose last part contains 'ostudy-ui-screenshots'."

# One capture at a time: the application data path and driver ports are fixed.
exec 8>"$RUN_HOME.lock"
if ! flock -n 8; then
  log "Waiting for another screenshot capture to finish"
  flock 8
fi
rm -rf -- "$RUN_HOME"
mkdir -p "$RUN_HOME"/{data,config,cache,state,runtime}
chmod 0700 "$RUN_HOME/runtime"
trap 'rm -rf -- "$RUN_HOME"' EXIT

mapfile -t LOCALE_ENV < <(capture_locale_env)

log "Capturing with $APPLICATION"
# Software rendering without accelerated compositing, as in native smoke:
# WebKit's GPU probing under Xvfb can reset WebKitWebDriver mid-session.
# The session bus points nowhere so the capture can never read or write the
# desktop keyring; device sync shows its "secure store unavailable" notice.
env \
  DBUS_SESSION_BUS_ADDRESS="unix:path=$RUN_HOME/no-session-bus" \
  NO_AT_BRIDGE=1 \
  XDG_DATA_HOME="$RUN_HOME/data" \
  XDG_CONFIG_HOME="$RUN_HOME/config" \
  XDG_CACHE_HOME="$RUN_HOME/cache" \
  XDG_STATE_HOME="$RUN_HOME/state" \
  XDG_RUNTIME_DIR="$RUN_HOME/runtime" \
  OSTUDY_PDFIUM_PATH="$PDFIUM_DIR" \
  TZ=UTC \
  "${LOCALE_ENV[@]}" \
  GDK_BACKEND=x11 \
  WEBKIT_DISABLE_COMPOSITING_MODE=1 \
  WEBKIT_DISABLE_DMABUF_RENDERER=1 \
  LIBGL_ALWAYS_SOFTWARE=1 \
  xvfb-run -a --server-args="-screen 0 1920x1200x24 -dpi 96 -nolisten tcp" \
  timeout --signal=INT "${CAPTURE_TIMEOUT}s" \
  python3 -u "$ENGINE" capture \
  --application "$APPLICATION" \
  --output "$OUTPUT" \
  --source-json "$SOURCE" \
  "${FILTERS[@]}"

#!/usr/bin/env bash

OSTUDY_ROOT_DIR="${OSTUDY_ROOT_DIR:-$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)}"
OSTUDY_BINARY="$OSTUDY_ROOT_DIR/src-tauri/target/debug/ostudy"
OSTUDY_BUILD_STAMP="$OSTUDY_ROOT_DIR/.ostudy-build-stamp"

ostudy_source_fingerprint() {
  (
    cd "$OSTUDY_ROOT_DIR"
    {
      for path in \
        package.json pnpm-lock.yaml pnpm-workspace.yaml index.html \
        vite.config.ts \
        tsconfig.json tsconfig.app.json tsconfig.node.json \
        src-tauri/Cargo.toml src-tauri/Cargo.lock \
        src-tauri/tauri.conf.json src-tauri/build.rs; do
        [[ -f "$path" ]] && printf '%s\0' "$path"
      done
      find src -type f \
        ! -name '*.test.ts' \
        ! -name '*.test.tsx' \
        ! -path 'src/test/*' \
        -print0
      find src-tauri/src src-tauri/migrations src-tauri/capabilities \
        -type f -print0
    } | sort -z | xargs -0 sha256sum | sha256sum | awk '{print $1}'
  )
}

ostudy_write_build_stamp() {
  local version="$1"
  local fingerprint
  fingerprint="$(ostudy_source_fingerprint)"
  cat > "$OSTUDY_BUILD_STAMP" <<STAMP
root=$OSTUDY_ROOT_DIR
binary=$OSTUDY_BINARY
version=$version
fingerprint=$fingerprint
STAMP
}

ostudy_stamp_value() {
  local key="$1"
  sed -n "s/^${key}=//p" "$OSTUDY_BUILD_STAMP" | head -n 1
}

ostudy_require_current_build() {
  if [[ ! -x "$OSTUDY_BINARY" ]]; then
    printf 'ERROR: ostudy has not been built in this checkout. Run ./scripts/build.sh.\n' >&2
    return 1
  fi
  if [[ ! -f "$OSTUDY_BUILD_STAMP" ]]; then
    printf 'ERROR: The build stamp is missing. Run ./scripts/build.sh once.\n' >&2
    return 1
  fi
  if [[ "$(ostudy_stamp_value root)" != "$OSTUDY_ROOT_DIR" ]]; then
    printf 'ERROR: This checkout moved after it was built. Run ./scripts/build.sh; stale Cargo artifacts will be cleared automatically.\n' >&2
    return 1
  fi

  local current_fingerprint recorded_fingerprint
  current_fingerprint="$(ostudy_source_fingerprint)"
  recorded_fingerprint="$(ostudy_stamp_value fingerprint)"
  if [[ "$current_fingerprint" != "$recorded_fingerprint" ]]; then
    printf 'ERROR: Source files changed after the last build. Run ./scripts/build.sh (or ./scripts/check.sh) before launching.\n' >&2
    return 1
  fi
}

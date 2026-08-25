#!/usr/bin/env bash

OSTUDY_ROOT_DIR="${OSTUDY_ROOT_DIR:-$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)}"
if ! declare -F ostudy_default_cargo_target_dir >/dev/null 2>&1; then
  # shellcheck disable=SC1091
  source "$OSTUDY_ROOT_DIR/scripts/cargo-target.sh"
fi
OSTUDY_BUILD_TARGET_DIR="${CARGO_TARGET_DIR:-$(ostudy_default_cargo_target_dir "$OSTUDY_ROOT_DIR")}"
if [[ "$OSTUDY_BUILD_TARGET_DIR" != /* ]]; then
  OSTUDY_BUILD_TARGET_DIR="$OSTUDY_ROOT_DIR/$OSTUDY_BUILD_TARGET_DIR"
fi

# Cargo's ordinary debug output is intentionally mutable. `tauri dev`, host
# Cargo checks/tests, and Android/Tauri materialization may all reuse that
# target directory after a standalone desktop build has completed. Never make
# the manual launcher or native acceptance tests depend on that mutable path.
OSTUDY_CARGO_BINARY="$OSTUDY_BUILD_TARGET_DIR/debug/ostudy"
OSTUDY_STANDALONE_DIR="$OSTUDY_BUILD_TARGET_DIR/standalone"
OSTUDY_BINARY="$OSTUDY_STANDALONE_DIR/ostudy"
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

ostudy_binary_sha256() {
  local path="$1"
  sha256sum -- "$path" | awk '{print $1}'
}

# Copy the Tauri CLI's just-built desktop executable to a path that ordinary
# Cargo/dev/Android work does not own. Native smoke and run-app.sh both launch
# this exact staged artifact, so later qualification cannot silently replace
# the executable that a human ultimately runs.
ostudy_stage_standalone_build() {
  if [[ ! -x "$OSTUDY_CARGO_BINARY" ]]; then
    printf 'ERROR: Expected Tauri desktop output was not created at %s.\n' "$OSTUDY_CARGO_BINARY" >&2
    return 1
  fi

  mkdir -p "$OSTUDY_STANDALONE_DIR"
  local temporary="$OSTUDY_STANDALONE_DIR/.ostudy.$$.tmp"
  rm -f -- "$temporary"
  cp -- "$OSTUDY_CARGO_BINARY" "$temporary"
  chmod 0755 "$temporary"
  mv -f -- "$temporary" "$OSTUDY_BINARY"
}

ostudy_write_build_stamp() {
  local version="$1"
  local fingerprint binary_sha

  if [[ ! -x "$OSTUDY_BINARY" ]]; then
    printf 'ERROR: Cannot stamp a missing standalone build artifact: %s\n' "$OSTUDY_BINARY" >&2
    return 1
  fi

  fingerprint="$(ostudy_source_fingerprint)"
  binary_sha="$(ostudy_binary_sha256 "$OSTUDY_BINARY")"
  cat > "$OSTUDY_BUILD_STAMP" <<STAMP
artifactKind=standalone
root=$OSTUDY_ROOT_DIR
binary=$OSTUDY_BINARY
compiledBinary=$OSTUDY_CARGO_BINARY
binarySha256=$binary_sha
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
    printf 'ERROR: The standalone ostudy build is missing. Run ./scripts/build.sh (or ./scripts/check.sh).\n' >&2
    return 1
  fi
  if [[ ! -f "$OSTUDY_BUILD_STAMP" ]]; then
    printf 'ERROR: The build stamp is missing. Run ./scripts/build.sh once.\n' >&2
    return 1
  fi
  if [[ "$(ostudy_stamp_value artifactKind)" != "standalone" ]]; then
    printf 'ERROR: The build stamp predates the standalone-artifact boundary. Run ./scripts/build.sh again.\n' >&2
    return 1
  fi
  if [[ "$(ostudy_stamp_value root)" != "$OSTUDY_ROOT_DIR" ]]; then
    printf 'ERROR: This checkout moved after it was built. Run ./scripts/build.sh; stale Cargo artifacts will be cleared automatically.\n' >&2
    return 1
  fi
  if [[ "$(ostudy_stamp_value binary)" != "$OSTUDY_BINARY" ]]; then
    printf 'ERROR: The configured standalone artifact changed after the last build. Run ./scripts/build.sh again.\n' >&2
    return 1
  fi
  if [[ "$(ostudy_stamp_value compiledBinary)" != "$OSTUDY_CARGO_BINARY" ]]; then
    printf 'ERROR: The configured Cargo target changed after the last build. Run ./scripts/build.sh again.\n' >&2
    return 1
  fi

  local current_fingerprint recorded_fingerprint
  current_fingerprint="$(ostudy_source_fingerprint)"
  recorded_fingerprint="$(ostudy_stamp_value fingerprint)"
  if [[ "$current_fingerprint" != "$recorded_fingerprint" ]]; then
    printf 'ERROR: Source files changed after the last build. Run ./scripts/build.sh (or ./scripts/check.sh) before launching.\n' >&2
    return 1
  fi

  local current_binary_sha recorded_binary_sha
  current_binary_sha="$(ostudy_binary_sha256 "$OSTUDY_BINARY")"
  recorded_binary_sha="$(ostudy_stamp_value binarySha256)"
  if [[ -z "$recorded_binary_sha" || "$current_binary_sha" != "$recorded_binary_sha" ]]; then
    printf 'ERROR: The standalone build artifact changed after it was staged. Run ./scripts/build.sh (or ./scripts/check.sh) again.\n' >&2
    return 1
  fi
}

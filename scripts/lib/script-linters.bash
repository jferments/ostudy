#!/usr/bin/env bash

# Repository-owned acquisition for exact, checksum-verified script linters.
# Tools live outside the worktree so qualification remains source-read-only.

ostudy_verify_sha256() {
  local file="$1"
  local expected="$2"
  printf '%s  %s\n' "$expected" "$file" | sha256sum --check --strict - >/dev/null
}

ostudy_ensure_script_linters() {
  : "${RUFF_VERSION:?RUFF_VERSION must be loaded from versions.env first}"
  : "${SHELLCHECK_VERSION:?SHELLCHECK_VERSION must be loaded from versions.env first}"
  : "${TY_VERSION:?TY_VERSION must be loaded from versions.env first}"

  local platform arch ruff_asset ruff_checksum shellcheck_asset shellcheck_checksum ty_asset ty_checksum
  platform="$(uname -s)"
  arch="$(uname -m)"
  if [[ "$platform" != "Linux" ]]; then
    printf 'ERROR: pinned script-linter bootstrap currently supports Linux only; found %s.\n' "$platform" >&2
    return 1
  fi
  case "$arch" in
    x86_64)
      ruff_asset="ruff-x86_64-unknown-linux-gnu.tar.gz"
      ruff_checksum="$RUFF_LINUX_X86_64_SHA256"
      shellcheck_asset="shellcheck-v${SHELLCHECK_VERSION}.linux.x86_64.tar.xz"
      shellcheck_checksum="$SHELLCHECK_LINUX_X86_64_SHA256"
      ty_asset="ty-x86_64-unknown-linux-gnu.tar.gz"
      ty_checksum="$TY_LINUX_X86_64_SHA256"
      ;;
    aarch64|arm64)
      ruff_asset="ruff-aarch64-unknown-linux-gnu.tar.gz"
      ruff_checksum="$RUFF_LINUX_AARCH64_SHA256"
      shellcheck_asset="shellcheck-v${SHELLCHECK_VERSION}.linux.aarch64.tar.xz"
      shellcheck_checksum="$SHELLCHECK_LINUX_AARCH64_SHA256"
      ty_asset="ty-aarch64-unknown-linux-gnu.tar.gz"
      ty_checksum="$TY_LINUX_AARCH64_SHA256"
      ;;
    *)
      printf 'ERROR: unsupported script-linter architecture: %s.\n' "$arch" >&2
      return 1
      ;;
  esac

  local cache_root ruff_root shellcheck_root ty_root temp
  cache_root="${OSTUDY_TOOL_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/ostudy/tooling}"
  ruff_root="$cache_root/ruff-${RUFF_VERSION}-${arch}"
  shellcheck_root="$cache_root/shellcheck-${SHELLCHECK_VERSION}-${arch}"
  ty_root="$cache_root/ty-${TY_VERSION}-${arch}"
  mkdir -p "$cache_root"

  if [[ ! -x "$ruff_root/ruff" ]] || [[ "$("$ruff_root/ruff" --version 2>/dev/null || true)" != "ruff $RUFF_VERSION" ]]; then
    temp="$(mktemp -d -t ostudy-ruff.XXXXXXXX)"
    curl --fail --location --retry 3 \
      --output "$temp/$ruff_asset" \
      "https://github.com/astral-sh/ruff/releases/download/${RUFF_VERSION}/$ruff_asset"
    ostudy_verify_sha256 "$temp/$ruff_asset" "$ruff_checksum"
    rm -rf -- "$ruff_root"
    mkdir -p "$ruff_root"
    tar -xzf "$temp/$ruff_asset" -C "$ruff_root" --strip-components=1
    rm -rf -- "$temp"
  fi

  if [[ ! -x "$shellcheck_root/shellcheck" ]] || \
     [[ "$("$shellcheck_root/shellcheck" --version 2>/dev/null | sed -n 's/^version: //p' | head -n 1)" != "$SHELLCHECK_VERSION" ]]; then
    temp="$(mktemp -d -t ostudy-shellcheck.XXXXXXXX)"
    curl --fail --location --retry 3 \
      --output "$temp/$shellcheck_asset" \
      "https://github.com/koalaman/shellcheck/releases/download/v${SHELLCHECK_VERSION}/$shellcheck_asset"
    ostudy_verify_sha256 "$temp/$shellcheck_asset" "$shellcheck_checksum"
    rm -rf -- "$shellcheck_root"
    mkdir -p "$shellcheck_root"
    tar -xJf "$temp/$shellcheck_asset" -C "$shellcheck_root" --strip-components=1
    rm -rf -- "$temp"
  fi

  if [[ ! -x "$ty_root/ty" ]] ||      [[ "$("$ty_root/ty" version 2>/dev/null || true)" != "ty $TY_VERSION" ]]; then
    temp="$(mktemp -d -t ostudy-ty.XXXXXXXX)"
    curl --fail --location --retry 3       --output "$temp/$ty_asset"       "https://github.com/astral-sh/ty/releases/download/${TY_VERSION}/$ty_asset"
    ostudy_verify_sha256 "$temp/$ty_asset" "$ty_checksum"
    rm -rf -- "$ty_root"
    mkdir -p "$ty_root"
    tar -xzf "$temp/$ty_asset" -C "$ty_root" --strip-components=1
    rm -rf -- "$temp"
  fi

  export OSTUDY_RUFF="$ruff_root/ruff"
  export OSTUDY_SHELLCHECK="$shellcheck_root/shellcheck"
  export OSTUDY_TY="$ty_root/ty"
  export PATH="$ruff_root:$shellcheck_root:$ty_root:$PATH"

  [[ "$("$OSTUDY_RUFF" --version)" == "ruff $RUFF_VERSION" ]] || {
    printf 'ERROR: pinned Ruff version verification failed.\n' >&2
    return 1
  }
  [[ "$("$OSTUDY_SHELLCHECK" --version | sed -n 's/^version: //p' | head -n 1)" == "$SHELLCHECK_VERSION" ]] || {
    printf 'ERROR: pinned ShellCheck version verification failed.\n' >&2
    return 1
  }
  [[ "$("$OSTUDY_TY" version)" == "ty $TY_VERSION" ]] || {
    printf 'ERROR: pinned ty version verification failed.\n' >&2
    return 1
  }
}

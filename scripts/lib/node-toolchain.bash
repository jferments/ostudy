#!/usr/bin/env bash

# Select the exact Node/npm pair pinned by versions.env without mutating the
# developer's global shell configuration. setup-ubuntu.sh installs the audited
# Node distribution under ~/.local/opt; qualification should prefer that stable
# installation even if ~/.local/bin/node is later repointed by another tool.

ostudy_node_bin_matches_pins() {
  local bin_dir="$1"
  [[ -x "$bin_dir/node" && -x "$bin_dir/npm" ]] || return 1

  local node_version npm_version
  node_version="$("$bin_dir/node" --version 2>/dev/null | sed 's/^v//')" || return 1
  npm_version="$("$bin_dir/npm" --version 2>/dev/null)" || return 1
  [[ "$node_version" == "$NODE_VERSION" && "$npm_version" == "$NPM_VERSION" ]]
}

ostudy_node_managed_bin() {
  local arch
  case "$(uname -m)" in
    x86_64) arch="x64" ;;
    aarch64|arm64) arch="arm64" ;;
    *) return 1 ;;
  esac
  printf '%s/.local/opt/node-v%s-linux-%s/bin\n' "$HOME" "$NODE_VERSION" "$arch"
}

ostudy_activate_node_toolchain() {
  : "${NODE_VERSION:?NODE_VERSION must be loaded from versions.env first}"
  : "${NPM_VERSION:?NPM_VERSION must be loaded from versions.env first}"

  local original_path="$PATH"
  local selected_bin=""
  local selected_source=""
  local candidate

  if [[ -n "${OSTUDY_NODE_HOME:-}" ]]; then
    candidate="$OSTUDY_NODE_HOME/bin"
    if ! ostudy_node_bin_matches_pins "$candidate"; then
      printf 'ERROR: OSTUDY_NODE_HOME=%s does not provide Node %s with npm %s.\n' \
        "$OSTUDY_NODE_HOME" "$NODE_VERSION" "$NPM_VERSION" >&2
      return 1
    fi
    selected_bin="$candidate"
    selected_source="OSTUDY_NODE_HOME"
  else
    candidate="$(ostudy_node_managed_bin 2>/dev/null || true)"
    if [[ -n "$candidate" ]] && ostudy_node_bin_matches_pins "$candidate"; then
      selected_bin="$candidate"
      selected_source="setup-managed"
    fi

    if [[ -z "$selected_bin" && -n "${NVM_DIR:-}" ]]; then
      candidate="$NVM_DIR/versions/node/v${NODE_VERSION}/bin"
      if ostudy_node_bin_matches_pins "$candidate"; then
        selected_bin="$candidate"
        selected_source="nvm"
      fi
    fi

    if [[ -z "$selected_bin" ]]; then
      candidate="$HOME/.nvm/versions/node/v${NODE_VERSION}/bin"
      if ostudy_node_bin_matches_pins "$candidate"; then
        selected_bin="$candidate"
        selected_source="nvm"
      fi
    fi

    if [[ -z "$selected_bin" ]]; then
      local path_entry
      local -a path_entries=()
      IFS=':' read -r -a path_entries <<< "$original_path"
      for path_entry in "${path_entries[@]}"; do
        [[ -n "$path_entry" ]] || continue
        if ostudy_node_bin_matches_pins "$path_entry"; then
          selected_bin="$path_entry"
          selected_source="PATH"
          break
        fi
      done
    fi
  fi

  if [[ -n "$selected_bin" ]]; then
    export PATH="$selected_bin:$HOME/.local/bin:$HOME/.cargo/bin:$original_path"
    export OSTUDY_NODE_TOOLCHAIN_BIN="$selected_bin"
    export OSTUDY_NODE_TOOLCHAIN_SOURCE="$selected_source"
    return 0
  fi

  # Do not download or rewrite toolchains during qualification. Preserve the
  # caller's PATH so doctor.sh can report the exact mismatch and remediation.
  export PATH="$original_path:$HOME/.local/bin:$HOME/.cargo/bin"
  export OSTUDY_NODE_TOOLCHAIN_BIN=""
  export OSTUDY_NODE_TOOLCHAIN_SOURCE="unresolved"
}

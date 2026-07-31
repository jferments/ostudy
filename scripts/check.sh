#!/usr/bin/env bash
set -Eeuo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
cd "$ROOT_DIR"

"$SCRIPT_DIR/doctor.sh"
node "$SCRIPT_DIR/verify-project-manifests.mjs"
[[ -f pnpm-lock.yaml && -f src-tauri/Cargo.lock ]] || {
  printf 'ERROR: lockfiles are missing. Run ./run-app.sh once first.\n' >&2
  exit 1
}

# Formatting and linting are developer quality gates. They deliberately live
# here instead of blocking every normal application launch.
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check

pnpm install --frozen-lockfile
pnpm run verify:versions
[[ "$(pnpm exec esbuild --version)" == "$ESBUILD_VERSION" ]]
pnpm run build

cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features
cargo build --manifest-path src-tauri/Cargo.toml --locked
"$SCRIPT_DIR/verify-rust-dependencies.sh"
printf '\nAll project checks passed.\n'

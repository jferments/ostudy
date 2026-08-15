#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

rm -rf -- dist
export VITE_OSTUDY_ANDROID_TEST_BRIDGE=1
pnpm exec vite build
node "$SCRIPT_DIR/verify-android-smoke-frontend.mjs" present

rm -rf -- dist
unset VITE_OSTUDY_ANDROID_TEST_BRIDGE || true
pnpm exec vite build
node "$SCRIPT_DIR/verify-android-smoke-frontend.mjs" absent

printf 'OK   Android smoke bridge include/exclude frontend preflight passed.\n'

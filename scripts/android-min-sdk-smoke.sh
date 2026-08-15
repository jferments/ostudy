#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-qualification-gates.sh"

ARTIFACT_DIR="${OSTUDY_ANDROID_MIN_SMOKE_ARTIFACTS:-$ROOT_DIR/artifacts/android-min-sdk-smoke}"
if [[ "${1:-}" == "--artifacts" ]]; then
  ARTIFACT_DIR="${2:?--artifacts requires a directory}"
  shift 2
fi
if (($#)); then
  printf 'Usage: ./scripts/android-min-sdk-smoke.sh [--artifacts DIR]\n' >&2
  exit 2
fi

cd "$ROOT_DIR"
bash ./scripts/install-android-deps.sh --with-min-emulator
rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  i686-linux-android \
  x86_64-linux-android
android_write_local_properties
if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
  android_emit_github_environment
fi
bash ./scripts/android-runner-doctor.sh --system-image "$ANDROID_MIN_SYSTEM_IMAGE"
pnpm install --frozen-lockfile
node ./scripts/verify-android-project.mjs
bash ./scripts/android-smoke-frontend-preflight.sh
bash ./scripts/android-build.sh --debug --apk --target x86_64 --smoke-bridge
bash ./scripts/android-gradle-contracts.sh
bash ./scripts/android-gradle-postbuild.sh test-packages
bash ./scripts/verify-build-cleanliness.sh

app_apk="$(ostudy_android_find_app_apk)"
test_apk="$(ostudy_android_find_test_apk)"
[[ -f "$app_apk" && -f "$test_apk" ]]
bash ./scripts/android-emulator-smoke.sh \
  --app-apk "$app_apk" \
  --test-apk "$test_apk" \
  --avd-name "$ANDROID_MIN_AVD" \
  --system-image "$ANDROID_MIN_SYSTEM_IMAGE" \
  --artifacts "$ARTIFACT_DIR"
bash ./scripts/verify-build-cleanliness.sh

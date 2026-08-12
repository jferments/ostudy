#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/android-env.sh"

RUN_EMULATOR=0
case "${1:-}" in
  "") ;;
  --emulator) RUN_EMULATOR=1 ;;
  -h|--help)
    printf 'Usage: ./scripts/android-test.sh [--emulator]\n'
    exit 0
    ;;
  *) printf 'Unknown option: %s\n' "$1" >&2; exit 2 ;;
esac

cd "$ROOT_DIR"
bash ./scripts/android-doctor.sh "$([[ $RUN_EMULATOR == 1 ]] && printf '%s' --emulator)"
android_write_local_properties
pnpm install --frozen-lockfile
node ./scripts/verify-android-project.mjs
pnpm exec tsc -b --pretty false
pnpm test
cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features
cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features

if ((RUN_EMULATOR == 1)); then
  exec bash ./scripts/android-emulator-smoke.sh
fi

export VITE_OSTUDY_ANDROID_TEST_BRIDGE=1
bash ./scripts/android-build.sh --debug --apk --target x86_64 --smoke-bridge
(
  cd src-tauri/gen/android
  ./gradlew --no-daemon :app:testDebugUnitTest :app:assembleDebugAndroidTest
)
app_apk="$({
  find src-tauri/gen/android/app/build/outputs/apk -type f -name '*.apk' \
    ! -path '*/androidTest/*' -printf '%T@ %p\n'
} | sort -nr | awk 'NR == 1 { $1=""; sub(/^ /, ""); print }')"
bash ./scripts/verify-android-package.sh "$app_apk" debug
printf 'OK   Android compile, unit-test, instrumentation-package, and APK checks passed.\n'

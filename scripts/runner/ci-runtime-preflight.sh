#!/usr/bin/env bash
set -Eeuo pipefail

EXPECTED_NAME="${OSTUDY_EXPECT_RUNNER_NAME:-ostudy-ci-unified}"
EXPECTED_USER="${OSTUDY_EXPECT_RUNNER_USER:-ostudy-runner}"
failures=0
warnings=0

ok() { printf 'OK    %s\n' "$*"; }
warn() { printf 'WARN  %s\n' "$*"; warnings=$((warnings + 1)); }
fail() { printf 'FAIL  %s\n' "$*" >&2; failures=$((failures + 1)); }

printf 'ostudy Actions runtime preflight\n'
printf '  runner: %s\n' "${RUNNER_NAME:-<unset>}"
printf '  os/arch: %s/%s\n' "${RUNNER_OS:-<unset>}" "${RUNNER_ARCH:-<unset>}"
printf '  user: %s\n\n' "$(id -un)"

[[ "${RUNNER_NAME:-}" == "$EXPECTED_NAME" ]] && ok "runner name is $EXPECTED_NAME" || fail "RUNNER_NAME is '${RUNNER_NAME:-<unset>}'; expected '$EXPECTED_NAME'"
[[ "${RUNNER_OS:-}" == "Linux" ]] && ok "runner OS is Linux" || fail "RUNNER_OS is '${RUNNER_OS:-<unset>}'; expected Linux"
[[ "${RUNNER_ARCH:-}" == "X64" ]] && ok "runner architecture is X64" || fail "RUNNER_ARCH is '${RUNNER_ARCH:-<unset>}'; expected X64"
[[ "$(id -un)" == "$EXPECTED_USER" ]] && ok "job runs as unprivileged $EXPECTED_USER" || fail "job user is $(id -un); expected $EXPECTED_USER"
[[ "$(id -u)" != 0 ]] && ok "job is not running as root" || fail "Actions jobs must never run as root"

if [[ -z "${GITHUB_WORKSPACE:-}" || -z "${RUNNER_WORKSPACE:-}" ]]; then
  fail "GITHUB_WORKSPACE and RUNNER_WORKSPACE must both be set"
else
  github_workspace="$(realpath -m "$GITHUB_WORKSPACE")"
  runner_workspace="$(realpath -m "$RUNNER_WORKSPACE")"
  case "$github_workspace/" in
    "$runner_workspace/"*) ok "GitHub workspace is contained by RUNNER_WORKSPACE" ;;
    *) fail "GITHUB_WORKSPACE escapes RUNNER_WORKSPACE: $github_workspace vs $runner_workspace" ;;
  esac
  if [[ "$(pwd -P)" == "$github_workspace" ]]; then
    ok "current directory is the checked-out GitHub workspace"
  else
    fail "current directory $(pwd -P) is not GITHUB_WORKSPACE $github_workspace"
  fi
fi

if findmnt -rn -t 9p,virtiofs,nfs,nfs4,cifs,fuse.sshfs 2>/dev/null | grep -q .; then
  fail "runner guest exposes a forbidden host/network filesystem mount"
else
  ok "runner guest exposes no forbidden host/network filesystem mounts"
fi

for socket_path in /var/run/docker.sock /run/docker.sock /var/run/libvirt/libvirt-sock /run/libvirt/libvirt-sock; do
  [[ -S "$socket_path" ]] && fail "forbidden host-control socket is present: $socket_path"
done

if command -v systemctl >/dev/null 2>&1; then
  mapfile -t runner_units < <(
    systemctl list-unit-files --type=service --no-legend 'actions.runner.*.service' 2>/dev/null |
      awk '{print $1}' |
      grep -F '.ostudy-ci-unified.service' || true
  )
  if (( ${#runner_units[@]} == 1 )); then
    restart_policy="$(systemctl show --property=Restart --value "${runner_units[0]}" 2>/dev/null || true)"
    if [[ "$restart_policy" == "always" ]]; then
      ok "runner service restart policy is always"
    else
      warn "runner service restart policy is '${restart_policy:-unknown}'; apply scripts/runner/install-service-hardening.sh while the runner is idle"
    fi
  else
    warn "could not identify exactly one ostudy-ci-unified systemd service"
  fi
fi

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    echo '## Runner runtime boundary'
    echo
    echo "- Runner: \`${RUNNER_NAME:-unknown}\`"
    echo "- OS/arch: \`${RUNNER_OS:-unknown}/${RUNNER_ARCH:-unknown}\`"
    echo "- User: \`$(id -un)\`"
    echo "- Isolation failures: \`$failures\`"
    echo "- Warnings: \`$warnings\`"
  } >> "$GITHUB_STEP_SUMMARY"
fi

if (( failures > 0 )); then
  printf '\nRuntime preflight failed: %d requirement(s) are not satisfied.\n' "$failures" >&2
  exit 1
fi
(( warnings > 0 )) && printf '\nRuntime preflight warnings: %d\n' "$warnings"
printf '\nOK    Actions runtime boundary verified.\n'

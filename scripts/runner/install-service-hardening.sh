#!/usr/bin/env bash
set -Eeuo pipefail

if [[ "$(id -u)" != 0 ]]; then
  printf 'ERROR: run this script with sudo/root privileges.\n' >&2
  exit 1
fi

RUNNER_NAME="${OSTUDY_EXPECT_RUNNER_NAME:-ostudy-ci-unified}"
RUNNER_USER="${OSTUDY_VM_RUNNER_USER:-ostudy-runner}"

if pgrep -u "$RUNNER_USER" -f 'Runner.Worker' >/dev/null 2>&1; then
  printf 'ERROR: a GitHub Actions Runner.Worker process is active. Wait for the runner to be idle before changing its service.\n' >&2
  exit 1
fi

mapfile -t units < <(
  systemctl list-unit-files --type=service --no-legend 'actions.runner.*.service' |
    awk '{print $1}' |
    grep -F ".${RUNNER_NAME}.service" || true
)
if (( ${#units[@]} != 1 )); then
  printf 'ERROR: expected exactly one Actions runner service for %s; found %d.\n' "$RUNNER_NAME" "${#units[@]}" >&2
  printf '%s\n' "${units[@]}" >&2
  exit 1
fi

unit="${units[0]}"
dropin_dir="/etc/systemd/system/${unit}.d"
install -d -m 0755 "$dropin_dir"
cat > "$dropin_dir/ostudy-resilience.conf" <<'EOF'
[Unit]
StartLimitIntervalSec=0

[Service]
Restart=always
RestartSec=5s
EOF

install -d -m 0755 /etc/needrestart/conf.d
cat > /etc/needrestart/conf.d/actions_runner_services.conf <<'EOF'
$nrconf{override_rc}{qr(^actions\.runner\..+\.service$)} = 0;
EOF

systemctl daemon-reload
systemctl restart "$unit"
systemctl is-active --quiet "$unit"

restart_policy="$(systemctl show --property=Restart --value "$unit")"
restart_delay="$(systemctl show --property=RestartUSec --value "$unit")"
if [[ "$restart_policy" != always ]]; then
  printf 'ERROR: %s Restart=%s after hardening; expected always.\n' "$unit" "$restart_policy" >&2
  exit 1
fi

printf 'OK    hardened %s\n' "$unit"
printf 'OK    Restart=%s RestartUSec=%s\n' "$restart_policy" "$restart_delay"
printf 'OK    needrestart runner-service override installed\n'

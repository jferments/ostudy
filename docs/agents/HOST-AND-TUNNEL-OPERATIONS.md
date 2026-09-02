# Host and Secure MCP Tunnel operations

This guide defines the host-administration boundary for agent-assisted oStudy
development on `codex-dev`. It supplements the repository workflow; it does not
grant an agent a generic shell or broader host privileges.

## Privileged-command policy

The `ubuntu` account must not be assumed to have an ordinary sudo password. The
host may authorize specific administrator-maintained commands through narrowly
scoped `NOPASSWD` policy while rejecting a generic credential-validation request.

Therefore:

- never run `sudo -v` on `codex-dev`;
- invoke only the exact intended privileged command;
- add `sudo -n` so missing authorization fails immediately instead of prompting;
- do not ask the human to invent, set, or enter an Ubuntu password;
- do not broaden sudoers merely to make an ad-hoc command work;
- do not hide an interactive sudo prompt behind a pipe carrying an API key.

Before requesting a secret, first prove that the exact non-secret command family
is available through the intended noninteractive sudo rule. Read a runtime key
from `/dev/tty`, not from pipeline standard input, and pipe it only to the exact
installer action that consumes it. Unset the shell variable on every exit path.

A safe pattern is:

```bash
if ! sudo -n -- /absolute/path/to/installer status >/dev/null 2>&1; then
  echo 'The exact installer is not authorized noninteractively.' >&2
  exit 1
fi

IFS= read -rsp 'Control-plane API key: ' CONTROL_PLANE_API_KEY </dev/tty
printf '\n' >/dev/tty
trap 'unset CONTROL_PLANE_API_KEY' EXIT

printf '%s' "$CONTROL_PLANE_API_KEY" |
  sudo -n -- /absolute/path/to/installer activate \
    --source-sha '<exact-qualified-sha>' \
    --tunnel-id 'tunnel_<32-lowercase-hex>' \
    --api-key-stdin
```

The installer path, SHA, tunnel ID, and supported arguments must come from the
qualified repository documentation rather than from memory.

## Multiple tunnel daemons on one VM

`codex-dev` may run several independent Secure MCP Tunnel daemons at once,
including direct workspace, observer, exact-head, and optional Codex-worker
profiles. A new profile must not accept a shared fixed local health/admin port.

Repository-managed profiles use:

```text
HEALTH_LISTEN_ADDR=127.0.0.1:0
```

Port `0` asks the kernel to allocate an unused loopback port for that process.
Each long-lived service writes the resolved private endpoint to its own
`HEALTH_URL_FILE`. The health listener is local diagnostics only; it does not
change the OpenAI tunnel ID, connector identity, or MCP tool contract.

Never stop an unrelated healthy tunnel merely to free `127.0.0.1:8080`. A new
profile that reports `bind: address already in use` must be regenerated with the
repository's collision-free listener policy.

## Interpreting a failed activation

The direct-workspace installer performs activation in this order:

1. replace the local tunnel profile;
2. run `tunnel-client doctor`;
3. persist the runtime API key in a root-only environment file;
4. install and start the systemd service.

A failure during step 2 can therefore leave the profile file present while no
API key or systemd service has been installed. That is a recoverable partial
state. Re-run the corrected idempotent activation with the same tunnel ID and a
runtime API key after updating to a qualified installer SHA. Do not delete
persistent worktrees or state, and do not stop unrelated services.

After successful activation, verify all of the following separately:

```text
systemd unit enabled
systemd unit active/running
no restart loop
private health URL file present
/healthz reachable
/readyz reports ready
ChatGPT exposes the exact expected MCP tool contract
```

A local health-listener failure is not evidence that GitHub authentication,
oStudy source, the MCP tool schema, or another tunnel failed.

## Ownership and escalation

Host service installation remains a human/operator boundary. Direct workspace
owns ordinary task-branch source changes after deployment. The observer remains
read-only; exact-head remains isolated validation; Codex remains optional under
[`EXECUTOR-SELECTION.md`](EXECUTOR-SELECTION.md).

When a host operation fails, report the exact command, exit status, owning
service/profile, listener or filesystem path, and whether earlier installer
phases completed. Do not collapse the result into a generic connectivity claim.

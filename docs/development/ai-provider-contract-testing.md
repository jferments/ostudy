# AI provider profile and contract testing

## Test tiers

The ordinary self-hosted qualification gate is completely deterministic. It does
not contact a real model server, require internet access, or consume a cloud API
key. Native provider behavior is exercised against bounded local HTTP fixtures.

```text
./scripts/check-static.sh
  manifest, migration, IPC, repository-policy, TypeScript, and formatting gates

./scripts/check-fast.sh
  static gate + React/Vitest + Rust check

./scripts/check.sh
  fast gate + production build + Clippy + Rust tests + native smoke
```

Same-repository pull requests also run a self-hosted rustfmt-normalization job.
It commits canonical Rust formatting only when the PR branch actually needs it.
Cargo lockfile changes remain explicit and reviewed rather than being silently
rewritten.

## Credential invariants

Tests use unique sentinel credentials and verify that they are absent from:

- AI profile and task-default tables;
- coherent SQLite backup snapshots;
- read DTO serialization;
- capability and probe records;
- safe error messages;
- frontend-rendered profile state.

The native keyring is not required by normal CI. Session-only credential storage
provides deterministic coverage of the secret boundary. A profile marked
session-only never falls through to a stale keyring credential after restart.
Native operating-system keyring qualification is an opt-in platform test because
Linux Secret Service, Windows Credential Manager, and Apple Keychain availability
depends on the runner session.

## Provider contract fixtures

`src-tauri/src/ai/provider_contract_tests.rs` starts a minimal loopback HTTP
server and verifies the exact initial adapter behavior.

### llama.cpp

- model discovery through `/v1/models`;
- model/server properties through `/props`;
- text completion through `/v1/chat/completions`;
- JSON-Schema response formatting;
- non-thinking request controls for bounded connectivity probes;
- chat-template presence is fingerprinted without retaining template text;
- declared image and context-window metadata remains distinct from active probes.

The model ID returned by `/v1/models` is the routing identity ostudy must send
back to the server. For a typical single-model server this may be the GGUF file
path rather than the Hugging Face repository name entered by the learner. If a
single llama.cpp model is returned under a different ID, Test connection treats
that as a recoverable partial result and continues with the server-reported ID.
Refresh models places that ID into the profile draft so it can be reviewed and
saved. Starting llama.cpp with an explicit `--alias` is another way to establish
a stable API model ID.

### DeepSeek

- current model discovery through `/models`;
- text completion through `/chat/completions`;
- current V4 model IDs such as `deepseek-v4-flash` are accepted from discovery
  rather than relying on retired hard-coded aliases;
- connectivity probes explicitly disable thinking and disable streaming;
- JSON object mode is used rather than false provider-enforced JSON Schema;
- strict Rust deserialization and request-specific semantic echo validation;
- valid JSON with incorrect semantics is rejected;
- valid exact final content may still pass if the provider reports a `length`
  finish reason.

Connectivity tests deliberately use non-thinking mode because their purpose is a
small deterministic final answer. Later learning tasks may explicitly enable
provider reasoning when the task policy requires it.

### Transport

- redirects are rejected rather than followed with a bearer credential;
- provider responses and errors remain bounded;
- normalized errors never include the credential sentinel.

## Redacted diagnostics

After Test connection, **View diagnostic information** exposes only metadata
safe to share in a bug report:

- provider and normalized endpoint;
- Local/LAN/Cloud destination scope;
- configured model and model IDs refreshed during the current UI session;
- adapter and result-schema revisions;
- persisted/stale state;
- per-phase status, latency, normalized error code, and retryability.

It deliberately excludes API credentials, Authorization headers, raw prompts,
provider response text, model reasoning, and complete llama.cpp chat templates.
When asking for connection debugging information, prefer this panel over copying
raw provider traffic.

## Manual llama.cpp qualification

After the deterministic gate passes, a developer may configure a real profile in
Settings → AI/LLM:

1. Start an existing llama.cpp server outside ostudy.
2. Add a **llama.cpp / Unsloth GGUF** profile with its loopback or explicitly
   acknowledged LAN URL.
3. Save the profile, then choose **Refresh models**.
4. If the server reports one different model ID, review the ID placed into the
   Model ID field and save it. Alternatively configure a llama.cpp `--alias` and
   refresh again.
5. Run **Test connection**.
6. Confirm model discovery, server properties when available, text completion,
   and structured output.
7. Confirm the server/model chat-template capability is present when expected.
8. Open **View diagnostic information** and confirm no raw prompt, response, or
   credential is present.

A model exported or fine-tuned with Unsloth is still configured as a llama.cpp
profile. ostudy does not generate or replace the model's token-level chat
template.

## Manual DeepSeek qualification

1. Add a **DeepSeek** profile.
2. Save the API key securely or choose session-only storage.
3. Refresh the current model list.
4. Select the desired current V4 model and save the profile.
5. Run **Test connection**.
6. Confirm text completion and JSON-object/local-validation capability.
7. Confirm strict provider JSON Schema remains marked unsupported.
8. Open **View diagnostic information** and confirm the probe is described as
   non-thinking and contains no credential or model output.

Testing sends tiny prompts and may consume a small amount of paid API usage. It
does not upload study documents or retain raw provider content.

## Expected failure categories

The UI should distinguish at least:

- incomplete or stale profile configuration;
- missing or rejected credential;
- secure-store unavailability;
- DNS/connection failure;
- timeout or TLS failure;
- rejected redirect;
- model-list mismatch, including the recoverable sole-llama identity case;
- rate limit and optional retry-after;
- server unavailable;
- empty or semantically incorrect final completion;
- invalid JSON;
- structured output that fails local semantic validation.

A provider test may be **partial** when optional model identity reconciliation,
model properties, or structured-output support is unavailable while ordinary
text completion works. A partial result is not silently promoted to full
readiness for a task requiring the missing capability.

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
- frontend-rendered profile state and diagnostic transcripts.

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

- provider-backed model discovery through `/models`;
- optional model/server properties through `/props?model=<selected-id>`;
- non-JSON or missing optional properties metadata does not invalidate an
  otherwise healthy provider connection;
- text completion through `/v1/chat/completions`;
- JSON-Schema response formatting;
- non-thinking request controls for bounded connectivity probes;
- chat-template presence is fingerprinted without retaining template text;
- declared image and context-window metadata remains distinct from active probes.

The model ID returned by the model-list API is the routing identity ostudy must
send back to the server. It may be a GGUF path rather than a Hugging Face
repository name. If a single llama.cpp model is returned under a different ID,
Test connection treats that as a recoverable partial result and continues with
the server-reported ID. Refresh models puts that ID into the model dropdown draft
so it can be reviewed and saved. Starting llama.cpp with an explicit `--alias`
is another way to establish a stable API model ID.

### DeepSeek

- current model discovery through `/models`;
- text completion through `/chat/completions`;
- current V4 model IDs are selected from provider discovery rather than typed or
  hard-coded in Settings;
- connectivity probes explicitly disable thinking and disable streaming;
- JSON Object mode is used rather than falsely claiming provider-enforced JSON
  Schema;
- strict Rust deserialization and request-specific semantic echo validation;
- valid JSON with incorrect semantics is rejected;
- valid exact final content may still pass if the provider reports a `length`
  finish reason.

Connectivity tests deliberately use non-thinking mode because their purpose is a
small deterministic final answer. Later learning tasks may explicitly enable
provider reasoning when the task policy requires it.

The DeepSeek capability display must show provider JSON Schema as **not offered**,
not as a red connection error. The independent **Local validation** capability is
the evidence that ostudy checked the provider's JSON against the expected schema.

### Transport

- redirects are rejected rather than followed with a bearer credential;
- provider responses and errors remain bounded;
- normalized errors never include the credential sentinel.

## Plaintext redacted diagnostics

After Test connection, the UI renders a selectable **Connection diagnostic log**
as plain text rather than a collapsible metadata panel. It is intended to be
copied directly into a bug report.

The transcript includes:

- provider and normalized endpoint;
- Local/LAN/Cloud destination scope;
- configured and tested model IDs;
- model discovery and completion routes attempted;
- optional llama.cpp properties route;
- adapter and result-schema revisions;
- per-phase status, latency, normalized error code, and retryability;
- a bounded preview of model IDs refreshed during the current UI session.

It deliberately excludes API credentials, Authorization headers, raw prompts,
provider response text, model reasoning, and complete llama.cpp chat templates.
When asking for connection debugging information, copy this plaintext transcript
rather than capturing raw provider traffic.

## Manual llama.cpp qualification

After the deterministic gate passes, a developer may configure a real profile in
Settings → AI/LLM:

1. Start an existing llama.cpp server outside ostudy.
2. Add a **llama.cpp / Unsloth GGUF** profile with its loopback or explicitly
   acknowledged LAN URL and create the profile.
3. Choose **Refresh models**. Confirm the Model dropdown is populated by the
   server rather than requiring free-text input.
4. Select the intended model. If the server exposes a path-like model ID, either
   save that returned ID or configure a llama.cpp `--alias` and refresh again.
5. Save the selected model and run **Test connection**.
6. Confirm model discovery, text completion, and structured output. Properties
   metadata may remain unavailable without degrading those verified capabilities.
7. Confirm the server/model chat-template capability is reported when properties
   metadata actually exposes it.
8. Copy the **Connection diagnostic log** and confirm it contains attempted routes
   and phase statuses but no raw prompt, response, or credential.

A model exported or fine-tuned with Unsloth is still configured as a llama.cpp
profile. ostudy does not generate or replace the model's token-level chat
template.

## Manual DeepSeek qualification

1. Add a **DeepSeek** profile and create it.
2. Save the API key securely or choose session-only storage.
3. Choose **Refresh models** and confirm the Model dropdown is populated by the
   current DeepSeek model-list API.
4. Select the desired model and save the profile.
5. Run **Test connection**.
6. Confirm text completion, JSON Object, and Local validation are supported.
7. Confirm **Provider JSON Schema** is displayed neutrally as **not offered** and
   is not treated as a failed connection.
8. Copy the **Connection diagnostic log** and confirm the probe is described as
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
- invalid JSON from a required provider response;
- structured output that fails local semantic validation.

Optional llama.cpp properties metadata is not a required provider response and
must not turn successful text/schema generation into a failed or partial
connection solely because `/props` is missing or returns non-JSON data.

A provider test may be **partial** when model identity reconciliation or required
structured-output support is unavailable while ordinary text completion works.
A partial result is not silently promoted to full readiness for a task requiring
the missing capability.

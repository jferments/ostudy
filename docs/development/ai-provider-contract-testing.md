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

## Credential invariants

Tests use unique sentinel credentials and verify that they are absent from:

- AI profile and task-default tables;
- coherent SQLite backup snapshots;
- read DTO serialization;
- capability and probe records;
- safe error messages;
- frontend-rendered profile state.

The native keyring is not required by normal CI. Session-only credential storage
provides deterministic coverage of the secret boundary. Native operating-system
keyring qualification is an opt-in platform test because Linux Secret Service,
Windows Credential Manager, and Apple Keychain availability depends on the
runner session.

## Provider contract fixtures

`src-tauri/src/ai/provider_contract_tests.rs` starts a minimal loopback HTTP
server and verifies the exact initial adapter behavior:

### llama.cpp

- model discovery through `/v1/models`;
- model/server properties through `/props`;
- text completion through `/v1/chat/completions`;
- JSON-Schema response formatting;
- chat-template presence is fingerprinted without retaining template text;
- declared image and context-window metadata remains distinct from active probes.

### DeepSeek

- model discovery through `/models`;
- text completion through `/chat/completions`;
- JSON object mode rather than false provider-enforced JSON Schema;
- strict Rust deserialization and request-specific semantic echo validation;
- valid JSON with incorrect semantics is rejected.

### Transport

- redirects are rejected rather than followed with a bearer credential;
- provider responses and errors remain bounded;
- normalized errors never include the credential sentinel.

## Manual llama.cpp qualification

After the deterministic gate passes, a developer may configure a real profile in
Settings → AI/LLM:

1. Start an existing llama.cpp server outside ostudy.
2. Add a **llama.cpp / Unsloth GGUF** profile.
3. Use the loopback or explicitly acknowledged LAN URL.
4. Refresh models, select the intended loaded model ID, and save.
5. Run **Test connection**.
6. Confirm endpoint/model discovery, text completion, and structured output.
7. Confirm the server/model chat-template capability is present when expected.

A model exported or fine-tuned with Unsloth is still configured as a llama.cpp
profile. ostudy does not generate or replace the model's token-level chat
template.

## Manual DeepSeek qualification

1. Add a **DeepSeek** profile.
2. Save the API key securely or choose session-only storage.
3. Refresh the current model list.
4. Select a model and save the profile.
5. Run **Test connection**.
6. Confirm text completion and JSON-object/local-validation capability.
7. Confirm strict provider JSON Schema remains marked unsupported.

Testing sends a tiny prompt and may consume a small amount of paid API usage.
The test does not upload study documents or retain raw provider content.

## Expected failure categories

The UI should distinguish at least:

- incomplete or stale profile configuration;
- missing or rejected credential;
- secure-store unavailability;
- DNS/connection failure;
- timeout or TLS failure;
- rejected redirect;
- missing model;
- rate limit and optional retry-after;
- server unavailable;
- empty or truncated completion;
- invalid JSON;
- structured output that fails local semantic validation.

A provider test may be **partial** when optional model discovery/properties or
structured-output support is unavailable while ordinary text completion works.
A partial result is not silently promoted to full readiness for a task requiring
the missing capability.

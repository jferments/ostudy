# AI provider platform guardrails

These rules apply to every future ostudy feature that uses an LLM, embedding
model, reranker, OCR model, speech model, or another remote/local AI endpoint.
They extend the general architecture, code, UI, and validation guardrails.

## Native trust boundaries

- All provider traffic originates in `src-tauri/src/ai/` or a task-specific Rust
  service that calls the internal AI gateway.
- React, controllers, shared UI, and `src/platform/` must not use `fetch`,
  `XMLHttpRequest`, provider JavaScript SDKs, API credentials, TLS settings, or
  provider response parsing.
- `reqwest` remains confined to `src-tauri/src/ai/gateway/`. Provider protocol
  modules describe routes and wire contracts; they do not create independent clients.
- Operating-system credential access remains confined to
  `src-tauri/src/ai/secrets.rs`.
- Never add credential bytes, masked credentials, reversible encrypted secrets,
  secret hashes, raw authorization headers, or secret references supplied by the
  frontend to SQLite.
- Credentials may not appear in `.studydeck`, `.ostudy.json`, `.ostudybackup`,
  diagnostics, logs, analytics, operation payloads, probe records, or read DTOs.

## Profiles and endpoints

- Provider profiles are durable entities separate from `AppSettings`.
- Provider kind is immutable. A different protocol or trust boundary is a new
  profile, not an in-place reinterpretation.
- Built-in cloud providers use fixed official endpoints unless an explicitly
  designed custom-provider type is added later.
- Only validated `http` or `https` endpoints are allowed. Public cleartext HTTP,
  URL user information, query strings, fragments, arbitrary schemes, redirect
  following, and TLS-verification bypasses remain forbidden.
- Loopback HTTP is allowed. LAN HTTP requires an explicit user acknowledgement.
- Profile, endpoint, authentication, task-default, and credential mutations use
  the shared AI mutation lane, optimistic revisions, and operation IDs.
- Never silently fall back from a local profile to a LAN or cloud profile.

## Provider protocol family

- The implemented llama.cpp, DeepSeek, and OpenRouter integrations share the
  OpenAI-compatible protocol family under `src-tauri/src/ai/providers/`.
- Shared model-list parsing, Chat Completions fields, response parsing, structured
  output decoding, usage normalization, and representative document probing must
  not be copied into per-provider modules.
- `providers/dialect.rs` is the single dispatch seam for current provider wire
  differences. Add custom code only for a demonstrated protocol difference.
- `providers/llama_cpp.rs` owns local route normalization, `/props`, chat-template
  controls, and llama.cpp structured-output compatibility.
- `providers/deepseek.rs` owns DeepSeek thinking controls and the beta strict-tool
  document contract.
- `providers/openrouter.rs` owns router/privacy preferences, account-filtered
  model metadata, and OpenRouter-specific document routing policy.
- Document intelligence owns task semantics, source grounding, and output
  validation; it must not branch on provider kind or construct provider wire
  vocabulary.
- Do not introduce an omnibus provider trait that simply mirrors every feature.
  Add another protocol family only when a provider is genuinely not compatible
  with the current shared protocol.

## Provider adapters

- An adapter owns provider routes, authentication expectations, wire DTOs,
  structured-output semantics, and safe error normalization.
- Do not scatter provider-name conditionals across task services or React.
- An OpenAI-compatible route is not proof of full OpenAI behavior. Capabilities
  remain `supported`, `unsupported`, or `unknown`, with `declared` versus
  `probed` evidence.
- llama.cpp or Ollama applies the model/server token-level chat template. ostudy
  owns task prompts and provider request envelopes, not arbitrary replacement
  Jinja/ChatML templates.
- Store at most a chat-template presence flag and one-way fingerprint. Never
  persist or return the full server template through ordinary profile APIs.
- Provider HTTP success is not output validity. Parse strict Rust DTOs and apply
  local structural and semantic validation.
- Raw model reasoning, chain-of-thought, prompts, responses, provider headers,
  and unknown provider metadata are not persisted by default.

## Task services

- Do not add a generic `send_ai_prompt`, `chat_with_ai`, or arbitrary completion
  IPC command.
- Each user feature owns a task-specific Rust service, versioned prompt,
  capability requirements, context limits, output contract, validation,
  persistence rules, and UI workflow.
- Providers never receive direct database, authoring, filesystem, shell, or Tauri
  command authority.
- AI-generated cards must pass the ordinary Rust authoring, safe-content,
  duplicate, tag, cloze, scheduling, and transaction boundaries.
- Future document content is untrusted data, not system instructions. Document
  text belongs in bounded source/user content blocks and cannot redefine tools or
  application policy.
- Network requests never run inside a SQLite transaction or while holding a
  database connection lock.

## UI and privacy

- Display the selected profile, model, and Local/LAN/Cloud destination before a
  task transmits content.
- Cloud use is explicit. Do not test providers at startup, on Settings open, or
  automatically after saving a key.
- Saved credentials are write-only: Set, Replace, Remove. There is no reveal or
  recover-existing-key action.
- Credential inputs clear immediately after submission and remain out of global
  React/application state.
- Capability and probe timestamps are historical evidence labeled “Last
  verified,” never a promise of a continuously live connection.
- Saving one Settings domain must not clear another domain's unsaved work.

## Tests and static enforcement

Every provider or task addition requires:

- deterministic local provider-contract fixtures requiring no internet or real
  credential;
- safe error and response-bound tests;
- secret sentinel tests covering DTOs, database bytes, backups, logs, and UI;
- endpoint and redirect tests;
- structured-output tests for malformed, truncated, semantically incorrect, and
  provider-refusal responses;
- stale profile/model/credential-epoch/**adapter-revision** tests;
- a representative document-contract probe that exercises the same provider
  dialect as real document generation;
- typed IPC parity and repository-policy updates;
- opt-in live-provider qualification separate from normal PR checks.

Do not weaken or bypass the static network, secret, migration, formatting,
lockfile, overlay, style-entry-point, or test-policy gates to land an AI feature.

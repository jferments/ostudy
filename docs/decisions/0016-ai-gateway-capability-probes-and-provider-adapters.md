# ADR 0016: Native AI gateway, capability probes, and initial providers

- **Status:** Accepted
- **Date:** 2026-08-07
- **Scope:** AI Stage 4.2

## Context

Provider APIs share broad concepts such as model identity, role messages,
completion text, structured output, and usage metadata, but their endpoints,
authentication, response shapes, error conventions, reasoning controls, and
structured-output semantics differ. Local OpenAI-compatible servers also
implement different subsets of nominally similar APIs. Scattering provider
conditionals across React or future flashcard/tutor features would expose
credentials, broaden the WebView network boundary, and make task validation easy
to bypass.

llama.cpp should apply the chat template stored with the model. DeepSeek uses
JSON Object mode rather than the same provider-enforced JSON Schema semantics
used by llama.cpp. HTTP success alone therefore cannot be treated as proof that a
capability works.

## Decision

### Rust-only gateway

All provider traffic originates in the native Rust AI subsystem. The WebView does
not use `fetch`, a Tauri HTTP plugin, or provider JavaScript SDKs. The Content
Security Policy remains closed to arbitrary provider endpoints. Credentials are
resolved inside Rust immediately before a request and never returned to React.

The native transport uses one bounded HTTP client with Rustls TLS, fixed user
agent, no cookies, disabled redirects, connect and request timeouts, bounded
response bodies, bounded error bodies, and no raw-body logging. A bearer token is
attached only to the normalized origin stored for that profile.

### Narrow command surface

Stage 4.2 exposes only profile-administration operations:

- list models for a saved profile;
- test a saved profile;
- read the latest redacted probe result.

There is no IPC command accepting arbitrary messages or returning arbitrary model
output. Future flashcard, quiz, and tutor services call the internal gateway
through task-specific native workflows that own prompts, grounding, schemas, and
persistence.

### Provider-backed model selection

Model identity is obtained from provider APIs rather than entered as arbitrary
free text in Settings. A user first creates the trusted profile/endpoint identity,
adds a credential when required, and then explicitly refreshes the provider's
model list. The frontend renders those returned IDs in a dropdown and saves only
the explicitly selected value.

The saved model remains visible if it is not present in a later refresh so an API
change cannot silently replace durable configuration. Testing a different model
requires selecting and saving it first.

### Provider adapters

The initial adapters are llama.cpp and DeepSeek. A shared OpenAI-style wire shape
is used only where the wire contracts overlap; provider adapters still own route
selection, authentication expectations, reasoning controls, optional endpoint
behavior, structured output, and capability interpretation.

For llama.cpp:

- `/models` is used for model discovery, including router deployments;
- `/props?model=<selected-id>` supplies optional declared model/server properties;
- missing or non-JSON optional properties metadata does not invalidate model
  discovery, text generation, or structured-output verification;
- `/v1/chat/completions` performs text and JSON-Schema probes;
- connectivity probes request non-thinking operation when supported;
- the server/model chat template remains authoritative;
- ostudy records only template presence and a SHA-256 fingerprint, never the
  template body;
- schema capability is considered supported only after local semantic
  validation;
- the API model ID is treated as a server-routing identity. A sole returned
  model with a different ID is a recoverable llama.cpp identity mismatch rather
  than immediate proof that the server cannot generate.

For DeepSeek:

- the fixed official endpoint and bearer credential are used;
- `/models` performs current model discovery;
- `/chat/completions` performs text and JSON-Object probes;
- bounded connectivity probes explicitly disable thinking so the output budget
  is spent on the final answer;
- the response is parsed into strict Rust data and checked against an exact text
  contract or request-specific structured echo value;
- DeepSeek is not advertised as provider-enforced JSON Schema;
- absence of provider-side JSON Schema is neutral capability metadata, while the
  separate local-validation capability records ostudy's schema enforcement.

A future task may intentionally enable model reasoning. The connectivity probe is
not a general task policy: it is deliberately small and deterministic.

### Model identity reconciliation

A saved provider model ID must normally match model discovery. The exception is a
single-model llama.cpp server: if model discovery returns exactly one different
ID, ostudy records a partial model-verification phase, temporarily uses the
server-reported ID for the remaining probe phases, and leaves the saved profile
unchanged. Refresh models puts that server ID into the model dropdown draft,
after which the user explicitly saves it. Starting llama.cpp with a stable alias
is the alternative way to keep a human-friendly API model ID.

Multiple-model mismatches and cloud-provider model mismatches remain hard
failures.

### Completion semantics

A provider finish reason is diagnostic metadata. `length` does not automatically
invalidate a probe if the returned final content already satisfies the exact
local contract. Empty content, wrong text, malformed JSON, or wrong semantic
echo still fail. This prevents a valid tiny answer from being discarded solely
because a reasoning-capable provider reported that it reached an internal output
budget boundary.

### Capability evidence

A profile test executes bounded phases: local profile validation, credential
resolution, model discovery, selected-model verification, optional server
properties, text completion, and structured output. Capabilities distinguish
`supported`, `unsupported`, and `unknown`, and whether evidence is `declared` or
`probed`.

Persisted results contain only bounded redacted fields: profile revision,
credential epoch, tested saved model ID, adapter revision, phase statuses, safe
errors, latencies, and capability states. Prompts, responses, reasoning content,
headers, credentials, and chat-template text are not stored.

Before persistence, the service reloads the profile and compares revision,
credential epoch, and saved model ID. Results from stale in-flight requests are
returned to the initiating caller but cannot replace current capability evidence.

### Plaintext redacted diagnostics

After a profile probe, Settings renders a selectable plaintext connection
transcript rather than a hidden disclosure panel. It contains only operational
metadata safe for the profile boundary: provider, normalized endpoint,
Local/LAN/Cloud scope, configured/tested model, attempted model/properties/
completion routes, adapter and result-schema revisions, a bounded model-list
preview, phase status/latency, and normalized safe error codes.

The transcript does not expose the credential, Authorization header, probe
prompt, provider response body, reasoning, or full chat template. Its purpose is
to let a user paste actionable connection/formatting evidence into a bug report
without capturing raw network traffic.

### Error normalization

Network and provider failures are normalized to stable safe codes covering
configuration, missing credentials, connection failures, timeouts, TLS,
redirects, oversized responses, authentication, permission, missing model,
rate limiting, provider availability, empty responses, invalid final text or
required JSON, and failed structured-output validation. Raw provider error bodies
are not part of the normal UI contract.

## Consequences

- future task services share one hardened transport and provider boundary;
- provider API drift is isolated to adapters and deterministic contract fixtures;
- model choices come from provider APIs rather than unvalidated hand-entered IDs;
- reasoning-capable models can be verified without wasting a tiny connectivity
  budget on hidden reasoning;
- local-server model aliases/file paths can be repaired without silently changing
  durable profile configuration;
- optional llama.cpp metadata cannot falsely turn successful generation into a
  provider failure;
- DeepSeek's lack of provider JSON Schema is represented accurately without
  implying that local structured-output validation failed;
- cloud tests occur only through an explicit user action and may consume a small
  amount of provider usage;
- no provider can gain card-writing or database authority through the gateway;
- adding Ollama, OpenAI, Gemini, or Claude requires an explicit adapter and
  capability tests rather than frontend conditionals.

## Non-goals

Stage 4.2 does not add PDF ingestion, a document library, flashcard generation,
interactive tutoring, embeddings, streaming tutor UI, general chat, long-running
AI jobs, provider fallback, or model-server installation.

## Validation

The decision is protected by deterministic local HTTP contract tests for
llama.cpp and DeepSeek, explicit reasoning-control request assertions, optional
llama properties fallback coverage, redirect and credential-forwarding tests,
wrong-semantic JSON rejection, response parser tests, llama model-identity
reconciliation tests, provider-backed model-dropdown tests, plaintext redacted
transcript tests, strict IPC parity, and repository policy checks confining
`reqwest` and keyring access to their native boundaries.

# ADR 0016: Native AI gateway, capability probes, and initial providers

- **Status:** Accepted
- **Date:** 2026-08-07
- **Scope:** AI Stage 4.2

## Context

Provider APIs share broad concepts such as model identity, role messages,
completion text, structured output, and usage metadata, but their endpoints,
authentication, response shapes, error conventions, and structured-output
semantics differ. Local OpenAI-compatible servers also implement different
subsets of nominally similar APIs. Scattering provider conditionals across React
or future flashcard/tutor features would expose credentials, broaden the WebView
network boundary, and make task validation easy to bypass.

llama.cpp should apply the chat template stored with the model. DeepSeek currently
supports JSON object mode rather than the same provider-enforced JSON Schema
semantics used by llama.cpp. HTTP success alone therefore cannot be treated as
proof that a capability works.

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

### Provider adapters

The initial adapters are llama.cpp and DeepSeek. A shared OpenAI-style wire shape
is used only where the wire contracts overlap; provider adapters still own route
selection, authentication expectations, optional endpoint behavior, structured
output, and capability interpretation.

For llama.cpp:

- `/v1/models` is used when available;
- `/props` supplies declared model/server properties;
- `/v1/chat/completions` performs text and JSON-Schema probes;
- the server/model chat template remains authoritative;
- ostudy records only template presence and a SHA-256 fingerprint, never the
  template body;
- schema capability is considered supported only after local semantic validation.

For DeepSeek:

- the fixed official endpoint and bearer credential are used;
- `/models` performs current model discovery;
- `/chat/completions` performs text and JSON-object probes;
- the response is parsed into a strict Rust DTO and checked against a
  request-specific echo value;
- DeepSeek is not advertised as provider-enforced JSON Schema.

### Capability evidence

A profile test executes bounded phases: local profile validation, credential
resolution, model discovery, selected-model verification, optional server
properties, text completion, and structured output. Capabilities distinguish
`supported`, `unsupported`, and `unknown`, and whether evidence is `declared` or
`probed`.

Persisted results contain only bounded redacted fields: profile revision,
credential epoch, tested model ID, adapter revision, phase statuses, safe errors,
latencies, capability states, and usage counts when returned. Prompts, responses,
reasoning content, headers, credentials, and chat-template text are not stored.

Before persistence, the service reloads the profile and compares revision,
credential epoch, and model ID. Results from stale in-flight requests are returned
to the initiating caller but cannot replace current capability evidence.

### Error normalization

Network and provider failures are normalized to stable safe codes covering
configuration, missing credentials, connection failures, timeouts, TLS,
redirects, oversized responses, authentication, permission, missing model,
rate limiting, provider availability, empty/truncated responses, invalid JSON,
and failed structured-output validation. Raw provider error bodies are not part
of the normal UI contract.

## Consequences

- future task services share one hardened transport and provider boundary;
- provider API drift is isolated to adapters and deterministic contract fixtures;
- local-server compatibility is represented as evidence rather than assumed from
  a provider label;
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
llama.cpp and DeepSeek, redirect and credential-forwarding tests, wrong-semantic
JSON rejection, response parser tests, stale-result tests, model-discovery and
capability UI tests, strict IPC parity, and repository policy checks confining
`reqwest` and keyring access to their native boundaries.

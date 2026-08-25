# ADR 0046: Modular AI gateway, profile, and capability-probe infrastructure

- **Status:** Accepted and implemented
- **Date:** 2026-08-24
- **Implementation:** PR #73, `agent/ai-infrastructure-modularization`
- **Base:** PR #72 squash `9448b32edea345b7c10dd6a0a77de73c8d645b82`

## Context

ostudy keeps AI provider access native-only. React may request narrowly defined
profile, verification, or learning-artifact operations, but it never receives
credentials, raw provider responses, arbitrary completions, or a generic prompt
interface.

Before this decision, three large Rust files owned most of the native provider
platform:

- `ai/gateway.rs` combined HTTP clients, request construction, ordinary JSON,
  cancellable SSE, inactivity timeouts, retries, usage normalization, provider
  metadata, safe errors, diagnostics, and tests;
- `ai/profiles.rs` combined serialized contracts, Tauri commands, profile CRUD,
  endpoint and privacy policy, operation replay, task defaults, credential-store
  coordination, SQL, projections, and tests; and
- `ai/probes.rs` combined serialized contracts, model discovery, ordered provider
  tests, phase policy, capability inference, persistence, freshness checks, and
  tests.

The behavior was already security-conscious, but ownership was difficult to
review. Pure parsing lived beside network execution; commands executed domain
and persistence logic; document services read profile and capability tables
directly; and ambient task state could silently change a generic JSON request
into the document streaming path.

PR #71 modularized the document and ingestion core. PR #72 replaced AnyDoc with
Xberg behind that application-owned ingestion boundary and strengthened sparse
source-evidence provenance. The AI infrastructure needed the same explicit
ownership without changing provider behavior or durable learner data.

## Decision

### 1. The three infrastructure roots are normal Rust packages

The retired root files do not remain as compatibility facades:

```text
src-tauri/src/ai/gateway.rs   (removed)
src-tauri/src/ai/profiles.rs  (removed)
src-tauri/src/ai/probes.rs    (removed)
```

Their replacements are:

```text
src-tauri/src/ai/
├── gateway/
├── profiles/
└── probes/
```

Each package has a narrow `mod.rs`, explicit child modules, focused tests, and an
architecture size tripwire. Path-attributed aliases and lexical source inclusion
are forbidden.

### 2. Ordinary and document provider requests are explicit

The gateway exposes two distinct behaviors:

```text
post_json(...)
    -> ordinary bounded JSON response

post_document_json(...)
    -> document-scoped cancellable SSE
    -> inactivity timeout between transport events
    -> bounded content and wire bytes
    -> streamed usage normalization
    -> at most one allowlisted transient retry
```

A generic request no longer inspects ambient document-task state to decide which
transport contract to use. Provider discovery and connection probes call the
ordinary path. Document intelligence deliberately calls the document path.

`post_document_json` falls back to ordinary JSON only when called outside a
document task, preserving safe internal/test use without changing the ordinary
entry point.

### 3. The gateway owns transport mechanics, not provider or task policy

The implemented topology is:

```text
ai/gateway/
├── mod.rs
├── client.rs
├── contracts.rs
├── request.rs
├── response.rs
├── retry.rs
├── errors.rs
├── metadata.rs
├── streaming/
│   ├── mod.rs
│   ├── sse.rs
│   └── usage.rs
└── tests/
```

Ownership is:

- `client.rs`: redirect-disabled ordinary and streaming `reqwest::Client`
  construction plus the ordinary JSON entry points;
- `request.rs`: common headers, bearer application, official OpenRouter
  attribution, and stream-option insertion;
- `response.rs`: content bounds, redirect rejection, status handling, bounded
  body reads, JSON decoding, and provider error-envelope detection;
- `streaming/mod.rs`: cancellable network loop, inactivity semantics, activity
  accounting, final normalization, and the single retry orchestration;
- `streaming/sse.rs`: pure incremental SSE reconstruction;
- `streaming/usage.rs`: provider-neutral usage normalization;
- `retry.rs`: the fixed transient retry allowlist and bounded delay;
- `errors.rs`: stable redacted transport/provider errors; and
- `metadata.rs`: bounded control-free diagnostic metadata.

The gateway does not depend on SQLite, profiles, probes, Tauri commands,
document semantics, authoring, cards, or study guides.

Provider adapters continue to own routes, provider request bodies, model lists,
structured-output semantics, and provider response decoding. Document
intelligence continues to own prompts, source grounding, task schemas,
educational output validation, and learning-artifact construction.

Document-only OpenRouter policy—such as disabling context compression, avoiding
sticky provider sessions, and excluding returned reasoning content—belongs in
the OpenRouter/document request builder rather than generic HTTP transport.

### 4. SSE parsing is a pure bounded state machine

`streaming/sse.rs` accepts arbitrary byte fragmentation and owns:

- LF and CRLF line boundaries;
- comments and keepalives;
- `[DONE]` termination;
- content deltas;
- compatible final full-message snapshots;
- reasoning byte accounting without retaining reasoning text;
- fragmented strict-tool arguments;
- stable tool-name enforcement;
- finish reasons;
- terminal usage envelopes;
- in-band provider failures; and
- independent content and wire-size limits.

It has no `reqwest`, Tokio, Tauri, SQLite, logging, cancellation-registry, or
profile dependency. Partial content is never returned after a transport,
provider, protocol, or cancellation failure.

### 5. Usage is normalized once at the transport boundary

The usage module accepts bounded nonnegative values from:

- OpenAI-style prompt/input, completion/output, and total token aliases;
- cached prompt and cache-write counters;
- reasoning-token counters;
- provider cost metadata; and
- llama.cpp timing/native token fields.

The normalized envelope feeds both task-local live progress and the existing
document usage accumulator. Source text, prompts, completions, and reasoning
content never enter usage metadata.

### 6. Safe errors and diagnostics remain redacted

Stable safe error codes remain part of the internal/UI compatibility contract.
The gateway classifies:

- connect, timeout, and generic network failures;
- HTTP status failures;
- OpenRouter/provider error envelopes;
- in-band stream errors;
- initial-response and inactivity timeouts;
- empty/truncated streams; and
- exact document cancellation.

Raw provider messages, bodies, headers, credentials, prompts, completions, and
reasoning are not copied into safe errors or routine diagnostics. Diagnostic
metadata is limited to bounded provider/model/generation identifiers, status and
error codes, retry counts, durations, byte counts, token/cost counters, and
bounded OpenRouter routing summaries.

### 7. Provider profiles are a bounded aggregate

The implemented profile package is:

```text
ai/profiles/
├── mod.rs
├── contracts.rs
├── commands.rs
├── service.rs
├── repository.rs
├── policy.rs
├── projections.rs
├── defaults.rs
└── tests/
```

Ownership is:

- `contracts.rs`: existing serialized inputs and DTOs plus crate-private request
  snapshots;
- `commands.rs`: Tauri state adaptation only;
- `service.rs`: mutation serialization, replay decisions, revision checks,
  secret-store coordination, repository calls, and post-commit projection;
- `repository.rs`: profile rows, operation identity, optimistic writes, editor
  draft participation, credential epochs, and transactional invalidation of
  derived probe evidence;
- `policy.rs`: provider/auth/privacy compatibility, normalization, bounds,
  revision validation, and trust-boundary rules;
- `projections.rs`: redacted summary/detail/configuration DTOs; and
- `defaults.rs`: task-default persistence and default profile request snapshots.

Provider kind remains immutable. Built-in cloud endpoints remain fixed. Loopback
HTTP is allowed, LAN HTTP requires explicit acknowledgement, and public remote
providers require HTTPS. A credential must be removed before changing endpoint
origin or disabling authentication.

Credential bytes are write-only and never enter repository inputs, SQLite,
operation records, DTOs, diagnostics, probe evidence, or backups. The established
secret-first, SQLite-metadata-second mutation ordering is preserved. This ADR
does not claim that two independent stores form an atomic transaction.

### 8. Capability probes separate orchestration, inference, and evidence

The implemented probe package is:

```text
ai/probes/
├── mod.rs
├── contracts.rs
├── commands.rs
├── runner.rs
├── phases.rs
├── capabilities.rs
├── repository.rs
└── tests/
```

The runner performs the existing ordered workflow:

1. load and revision-check a profile request snapshot;
2. validate the profile and resolve its credential;
3. discover models;
4. verify the selected model, including the sole-model llama.cpp compatibility
   rule;
5. read optional llama.cpp server properties;
6. run the minimal text probe;
7. run the structured-output probe;
8. run the representative OpenRouter document-generation schema probe when
   applicable;
9. infer normalized capabilities;
10. derive verified, partial, or failed status; and
11. persist only if profile revision, credential epoch, and model still match.

Network work does not hold a SQLite connection or transaction.

`capabilities.rs` is pure evidence-to-capability projection. Capabilities remain
three-valued (`supported`, `unsupported`, `unknown`) and record whether evidence
was declared or probed.

`repository.rs` owns selected-model declaration persistence, result loading,
freshness checks, bounded result JSON, atomic result/capability replacement, and
typed evidence readers.

### 9. Document services consume typed read models

Document task context no longer queries `ai_task_defaults`, profile credential
metadata, probe results, or capability rows directly. It consumes:

- a profile/default request snapshot; and
- a typed document-probe evidence projection.

Document provider planning no longer reads capability SQL directly. It consumes
typed persisted capability records and retains its provider-neutral planning
model, reasoning negotiation, structured-output strategy, completion ceiling,
and planning fingerprint.

This keeps AI persistence ownership inside profiles/probes while preserving the
document subsystem's provider-neutral behavior.

### 10. Public and durable compatibility is preserved

This refactor does not change:

- database schemas or migration checksums;
- Cargo or JavaScript dependencies;
- Tauri command names;
- camelCase IPC fields or enum spellings;
- provider routes or model discovery behavior;
- provider request envelopes or structured-output contracts;
- safe error codes;
- capability keys, evidence states, or persistence identity;
- prompt/schema versions or checkpoint identities;
- Xberg ingestion, canonical documents, sparse-evidence provenance, or PDF
  extraction/render/OCR behavior;
- learning plans, study guides, cards, schedules, reviews, or backup formats; or
- the frontend AI Settings workflow.

No generic prompt/chat IPC is added, and there is no provider fallback.

## Dependency direction

```text
React settings
    |
    v
typed AI-profile IPC
    |
    v
profiles::commands
    |
    v
profiles::service
    +--> profiles::policy
    +--> profiles::repository ------> SQLite profile aggregate
    +--> profiles::projections
    +--> secret manager ------------> OS/session secret store

probes::commands
    |
    v
probes::runner
    +--> provider adapters
    |        |
    |        v
    |    gateway -------------------> configured endpoint
    +--> probes::capabilities
    +--> probes::repository --------> SQLite verification evidence

document tasks
    +--> profile/default request snapshot
    +--> typed probe evidence
    +--> provider-neutral capability projection
    +--> explicit document gateway transport
```

Profiles do not depend on the gateway, providers, or probe runner. Probes consume
profile read models. Profile repository code may delete derived probe evidence
inside a source-profile mutation transaction, but it does not execute or infer
probes.

## Enforcement

`./scripts/check-static.sh` runs
`verify-ai-infrastructure-boundaries.mjs` and its Python fixture suite. The gate
rejects:

- the retired root monoliths;
- missing package modules and path aliases;
- replacement production modules above the size tripwire;
- `reqwest` or HTTP client construction outside the gateway owners;
- SQLite, Tauri, profile, probe, provider, document, or authoring dependencies in
  the gateway;
- Tauri commands outside profile/probe command adapters;
- profile/default SQL outside the profile aggregate;
- probe/capability SQL outside the probe repository, except explicit
  transactional invalidation by profile mutation;
- direct AI-table reads from document context/planning; and
- document transport that bypasses the explicit document gateway entry point.

The repository policy continues to forbid frontend provider networking, provider
SDKs, generic AI commands, credential persistence in SQLite, and unpinned or
unreviewed workflow behavior.

## Test contract

Deterministic tests cover:

- SSE fragmentation, keepalives, reasoning redaction, content/tool
  reconstruction, in-band failure, usage normalization, and byte bounds;
- ordinary versus document transport selection;
- cancellation while awaiting headers;
- cancellation after partial streamed output;
- cancellation during retry backoff;
- exactly one transient retry and successful retry reconstruction;
- redirects, status classification, metadata bounds, and raw-message redaction;
- profile serde contracts, policy, optimistic revisions, operation replay,
  capacity/name constraints, credential metadata, projections, and defaults;
- probe phase policy, model matching, capability inference, stale evidence,
  atomic replacement, size bounds, and typed readers;
- provider adapter contracts through local mock servers;
- credential sentinel absence from SQLite and backup bytes;
- frontend AI IPC/controller/settings compatibility; and
- full Linux/Tauri, native WebDriver, Android package, and API-36 emulator
  qualification.

Ordinary PR qualification requires no real provider or credential. Live-provider
acceptance remains explicit and human-controlled.

## Consequences

### Benefits

- Transport behavior can be reviewed and tested independently of profiles,
  probes, and document tasks.
- Profile and capability persistence have one runtime owner.
- Provider discovery cannot accidentally inherit document SSE/retry semantics.
- Document tasks cannot silently bypass the cancellable streaming contract.
- New provider envelopes have clear extension points for requests, responses,
  metadata, errors, and usage.
- Static checks prevent the three monoliths and duplicated ownership from
  returning.

### Costs

- There are more modules and some crate-private facade types.
- Credential mutations still cross two independent stores and retain their
  documented non-atomic ordering.
- Capability evidence remains durable JSON plus normalized rows rather than a
  more elaborate typed database schema.
- Human acceptance is still required for real local/LAN/cloud providers because
  deterministic fixtures cannot prove an external service's current behavior.

## Rollback

The change is source-only. There is no migration, dependency change, or data
conversion. A squash revert restores the former source topology while existing
profiles, credentials, probe evidence, documents, learning artifacts, cards,
schedules, reviews, and backups remain compatible with the unchanged durable
schemas.

## Superseded planning state

The initial proposed version of this ADR described PR #73 as a child of the
then-unmerged PR #72. PR #72 subsequently squash-merged as `9448b32e`, and the
implementation branch was rebuilt directly on that exact `main` head before the
source refactor and qualification proceeded. The accepted decision above is the
current architecture.
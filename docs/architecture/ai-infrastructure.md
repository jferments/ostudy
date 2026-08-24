# AI provider infrastructure

ostudy's AI platform is native-only and divided into three independently owned
subsystems: provider transport, durable provider profiles, and capability
verification. Feature-specific AI tasks, including document outlines,
flashcards, and study guides, consume these services but do not own credentials,
HTTP clients, provider capability persistence, or profile mutation policy.

The durable decision is recorded in
[ADR 0046](../decisions/0046-modular-ai-infrastructure.md).

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
    +--> native secret manager -----> OS/session secret store

probes::commands
    |
    v
probes::runner
    +--> provider adapters
    |        |
    |        v
    |    gateway transport ---------> configured endpoint
    +--> probes::capabilities
    +--> probes::repository --------> SQLite verification evidence

document tasks
    +--> profile/default request snapshot
    +--> typed probe evidence
    +--> provider-neutral capability projection
    +--> explicit document gateway transport
```

Dependencies point toward the native gateway and persistence owners. The gateway
has no knowledge of SQLite, profiles, probes, document semantics, cards, study
guides, or Tauri commands. Profile code does not call providers or run probes.
Probe execution consumes a profile snapshot and provider adapters, but profile
mutation never depends on the probe runner.

## Gateway package

`src-tauri/src/ai/gateway/` owns HTTP mechanics and safe transport behavior:

- `client.rs` constructs redirect-disabled ordinary and streaming `reqwest`
  clients and exposes bounded JSON operations;
- `request.rs` applies common headers, bearer authentication, OpenRouter
  attribution, streaming options, and document-request normalization;
- `response.rs` rejects redirects, enforces content bounds, decodes JSON, and
  detects provider error envelopes;
- `streaming/sse.rs` incrementally reconstructs content and strict-tool
  arguments from fragmented SSE bytes;
- `streaming/usage.rs` normalizes OpenAI-compatible and llama.cpp usage fields;
- `retry.rs` owns the single bounded retry used by document streams;
- `errors.rs` converts transport/provider failures into stable redacted errors;
- `metadata.rs` admits only bounded control-free provider metadata into
  diagnostics; and
- `contracts.rs` owns the small error/result surface shared with adapters.

Ordinary discovery and probe requests use a bounded JSON request. Long-running
document work uses the document transport contract: no total inference deadline,
a configurable inactivity timeout between transport events, exact document
cancellation, bounded response bytes, streamed usage, and at most one retry for
an allowlisted transient failure. Partial streamed output is never returned after
an error.

`reqwest` is forbidden outside this package. Provider modules build routes and
request envelopes but cannot create clients or bypass redirect, timeout,
response-bound, cancellation, or redaction policy.

## Profile aggregate

`src-tauri/src/ai/profiles/` separates the profile aggregate into:

- `contracts.rs`: serialized inputs/DTOs and crate-private request snapshots;
- `commands.rs`: Tauri adapters only;
- `service.rs`: mutation locking, operation replay, revision checks, secret-store
  coordination, and post-commit projection;
- `repository.rs`: profile rows, operation identity, optimistic writes, editor
  draft participation, credential epochs, and derived-evidence invalidation;
- `policy.rs`: provider/auth/privacy compatibility, field normalization,
  revision validation, and trust-boundary rules;
- `projections.rs`: summary/detail/configuration DTO construction; and
- `defaults.rs`: task-default persistence and default request snapshots.

Provider kind is immutable. Built-in cloud endpoints remain fixed. Loopback HTTP
is accepted, LAN HTTP requires explicit acknowledgement, and public remote
providers require HTTPS. A stored credential must be removed before changing
endpoint origin or disabling authentication.

Credentials are write-only. The service coordinates the OS/session secret store
and SQLite metadata, while the repository never receives credential bytes.
Credential changes advance a metadata epoch and invalidate derived verification
evidence. The established secret-first, metadata-second ordering is preserved;
this refactor does not pretend two independent stores form one atomic
transaction.

## Probe and capability evidence

`src-tauri/src/ai/probes/` contains:

- `contracts.rs`: stable phase, result, model, and capability DTOs;
- `commands.rs`: thin Tauri adapters;
- `runner.rs`: the ordered verification workflow;
- `phases.rs`: phase records and latency projection;
- `capabilities.rs`: pure evidence-to-capability inference; and
- `repository.rs`: result/capability persistence and typed readers.

The runner performs provider work without holding a database connection. It
validates the profile, resolves its credential, discovers and verifies the model,
reads optional server properties, tests text output, tests structured output, and
for OpenRouter tests the representative document-generation union schema. The
result is persisted only when the profile revision, credential epoch, and model
still match the tested snapshot.

Capabilities remain three-valued—`supported`, `unsupported`, or `unknown`—and
record whether evidence was declared or probed. A successful HTTP call alone
never proves a capability. Document task admission and planning consume typed
read models from the probe repository instead of querying capability tables
directly.

## Provider and task boundaries

Provider adapters continue to own provider-specific routes, model catalogs,
request bodies, structured-output semantics, and output decoding. Document
intelligence owns educational prompts, schemas, source grounding, semantic
validation, and learning-artifact construction. The gateway owns neither.

No subsystem exposes a generic prompt IPC command. React cannot receive
credentials, arbitrary completions, provider response bodies, or network
configuration. Network requests do not run inside SQLite transactions or while a
profile mutation lock is held.

## Privacy and diagnostics

The following never enter routine diagnostics, safe errors, SQLite, backup files,
or IPC DTOs:

- credentials and authorization headers;
- prompts and source material;
- generated response text;
- reasoning content;
- raw provider error messages; and
- unbounded unknown metadata.

Safe diagnostics may contain bounded provider/model/generation identifiers,
status/error codes, retry counts, duration, response byte counts, token/cost
counters, and bounded OpenRouter route summaries.

## Enforcement

`./scripts/check-static.sh` runs
`verify-ai-infrastructure-boundaries.mjs`. Its fixture suite rejects:

- the retired `gateway.rs`, `profiles.rs`, or `probes.rs` monoliths;
- replacement modules larger than the architectural tripwire;
- `reqwest` or client construction outside the gateway owners;
- Tauri command/state usage outside profile/probe command adapters;
- profile/default SQL outside the profile aggregate;
- probe/capability SQL outside the probe repository, except transactional
  invalidation by a profile mutation; and
- forbidden dependency directions between gateway, profiles, probes, providers,
  and persistence.

The verifier is a regression boundary, not a substitute for behavior tests. Each
provider addition still requires deterministic local contract fixtures, safe
error and response-bound tests, credential sentinels, stale-evidence tests, and
separate opt-in live-provider acceptance.

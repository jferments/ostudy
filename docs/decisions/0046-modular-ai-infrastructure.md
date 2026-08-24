# ADR 0046: Modular AI gateway, profile, and capability-probe infrastructure

- **Status:** Proposed for implementation
- **Date:** 2026-08-24
- **Parent pull request:** PR #72, `agent/xberg-document-adapter`
- **Parent head used to create the child branch:** `457451a3dbe27fe965c60c13ce94671cf944f093`
- **Implementation branch:** `agent/ai-infrastructure-modularization`

## Context

The document, ingestion, and parser boundaries were made explicit in PRs #71 and
#72. The next large-file refactor is the native AI infrastructure that those
document services depend on.

Three files currently carry most of the provider-platform behavior:

- `src-tauri/src/ai/gateway.rs` is approximately 67 KB and combines HTTP client
  construction, request policy, OpenRouter request mutation, ordinary JSON
  requests, cancellable SSE transport, inactivity timeouts, retry orchestration,
  incremental stream parsing, structured tool-call reconstruction, usage
  normalization, provider metadata projection, safe error classification,
  diagnostics, and tests.
- `src-tauri/src/ai/profiles.rs` is approximately 49 KB and combines serialized
  IPC contracts, Tauri commands, profile validation, endpoint/trust-boundary
  policy, profile CRUD, optimistic revisions, operation replay, task defaults,
  credential-store coordination, SQL row decoding, DTO projection, editor-draft
  commits, and tests.
- `src-tauri/src/ai/probes.rs` is approximately 41 KB and combines serialized IPC
  contracts, Tauri commands, model discovery, ordered probe execution, provider
  phase policy, capability inference, result persistence, capability-row
  persistence, freshness checks, profile loading, and tests.

The files are coherent enough that behavior is understandable, but their internal
ownership is not explicit. Pure parsing and policy tests sit beside Tauri state,
network execution sits beside provider-specific metadata interpretation, and
several downstream modules query the profile and probe tables directly instead
of consuming a stable AI-infrastructure read model.

The refactor must improve structure without changing the durable or user-visible
contract. In particular, PR #72's Xberg-backed document ingestion, sparse source
evidence rules, and provider-neutral learning-artifact pipeline must remain
unchanged.

## Goals

1. Replace the three root monoliths with normal Rust module packages whose files
   have one durable owner.
2. Preserve every existing Tauri command name, serialized field name, safe error
   code, provider route, provider request contract, retry limit, inactivity rule,
   response-size bound, capability key, persistence identity, and credential
   privacy rule.
3. Make ordinary provider requests and long-running document requests explicit
   gateway modes rather than selecting SSE through an implicit
   `current_signal().is_some()` branch inside the generic `post_json` method.
4. Keep provider HTTP construction inside the gateway package while keeping
   provider-specific request bodies in provider/document transport modules.
5. Centralize profile-table and probe/capability-table reads so document modules
   no longer reproduce AI-infrastructure SQL.
6. Add static architecture checks that prevent the three monoliths, hidden Tauri
   wrappers, unrestricted `reqwest`, and duplicated profile/probe table ownership
   from returning.
7. Add characterization coverage before moving behavior, then add focused unit
   and integration tests for each new owner.

## Non-goals

- No database migration or migration checksum change.
- No change to `ai_provider_profiles`, `ai_task_defaults`,
  `ai_profile_operations`, `ai_provider_probe_results`, or
  `ai_provider_capabilities` schemas.
- No new provider, model SDK, generic prompt command, parser integration, model
  fallback, endpoint fallback, or credential-storage mechanism.
- No AI settings redesign, profile-setup UX redesign, or semantic-coverage
  generation feature.
- No prompt, document schema, checkpoint, learning-plan, study-guide, generated
  card, backup, or IPC format change.
- No new Cargo or JavaScript dependency.
- No live-provider requirement in ordinary PR qualification.

## Dependency direction

The intended dependency graph is:

```text
Tauri commands
    |
    +--> profiles::commands --> profiles::service --> profiles::repository
    |                                  |                    |
    |                                  +--> policy          +--> SQLite
    |                                  +--> projections
    |                                  +--> secret manager
    |
    +--> probes::commands --> probes::runner --> provider adapters
                                      |             |
                                      |             v
                                      |         AI gateway
                                      |
                                      +--> probes::capabilities
                                      +--> probes::repository --> SQLite
                                      +--> profiles read model

provider adapters -------------------------------> AI gateway

document-intelligence transport ----------------> AI gateway
                 |                                profiles read model
                 +-------------------------------> probe/capability read model

document planning ------------------------------> probe/capability read model
```

The gateway must not depend on profiles, probes, SQLite, Tauri, document
semantics, or authoring. Profiles must not depend on gateway or providers. Probes
may consume the stable profile read model and provider adapters, but profile code
must not consume the probe runner. Profile mutations remain allowed to invalidate
probe-derived rows transactionally because the profile is the source aggregate
for that derived evidence.

## 1. Gateway package

The root `ai/gateway.rs` file will be removed and replaced with:

```text
src-tauri/src/ai/gateway/
├── mod.rs
├── contracts.rs
├── client.rs
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
    ├── mod.rs
    ├── request.rs
    ├── response.rs
    ├── streaming.rs
    ├── retry.rs
    └── redaction.rs
```

### `gateway/mod.rs`

The facade declares private children and re-exports the current internal public
surface:

- `AiGateway`
- `AiGatewayError`
- `SafeAiError`
- the safe `error(...)` constructor used by provider adapters

Existing imports such as `gateway::{AiGateway, AiGatewayError}` therefore remain
source-compatible. The facade contains no request execution or parsing logic.

### `gateway/contracts.rs`

Owns transport-level values that cross gateway package boundaries:

- `SafeAiError`
- `AiGatewayError`
- an explicit document-request policy/value used by the document transport
- bounded result metadata required by retry/diagnostics

The document request contract must not carry a document, semantic window, prompt
schema type, database handle, provider profile, or Tauri state. It contains only
transport facts such as the already-built URL/body, inactivity timeout,
response-size limit, and exact task cancellation scope.

### `gateway/client.rs`

Owns:

- the ordinary and streaming `reqwest::Client` instances;
- redirect-disabled client construction;
- connection timeout and user-agent policy;
- `get_json`;
- ordinary non-streaming `post_json`; and
- explicit `post_document_json` delegation.

`post_json` will no longer silently become a document SSE request merely because
a task-local cancellation signal exists. Provider discovery and connection
probes always use the ordinary bounded request path. Document intelligence calls
the explicit document method, which retains the current SSE, cancellation,
inactivity, retry, and usage behavior.

### `gateway/request.rs`

Owns provider-neutral request construction policy:

- common JSON/SSE Accept and Cache-Control headers;
- bearer application without exposing the secret;
- OpenRouter attribution headers for the fixed official HTTPS host;
- stream-option insertion;
- request-body size preparation; and
- fixed-host classification used for safe attribution.

The current document-only OpenRouter body mutation—disabling context compression
and suppressing returned reasoning—will move out of the generic gateway and into
the OpenRouter/document request builder. The resulting wire JSON must remain
identical. This removes task-specific prompt policy from the shared HTTP layer.

### `gateway/response.rs`

Owns:

- content-length preflight;
- bounded chunk accumulation;
- redirect rejection;
- status handling;
- JSON decoding;
- OpenRouter in-band error-envelope detection; and
- construction of the normalized Chat Completions-shaped result returned to
  provider/document code.

It does not interpret educational output or persist anything.

### `gateway/retry.rs`

Owns the one-retry document policy:

- the exact retryable error-code allowlist;
- `Retry-After` handling limited to the existing bounded range;
- the default 750 ms delay;
- cancellation during backoff;
- request/retry counters and safe diagnostics; and
- exhausted-retry message decoration.

There is no generic unbounded retry framework. Ordinary probes remain single
requests unless their provider adapter explicitly performs an existing
compatibility request.

### `gateway/errors.rs`

Owns every safe transport error classifier:

- `reqwest` timeout/connect/network failures;
- ordinary HTTP status mapping;
- OpenRouter/provider error envelopes;
- streaming in-band errors;
- inactivity, empty-stream, and initial-response failures;
- cancellation; and
- redacted error construction.

Raw provider messages, bodies, headers, credentials, prompts, completions, and
reasoning are never placed in `SafeAiError` or routine diagnostics. Existing
stable error codes remain unchanged. The stale PDF-specific cancellation prose
will be reviewed separately from the structural move; the refactor may generalize
that message only if every caller and UI test confirms the same code and more
accurate document-neutral behavior.

### `gateway/metadata.rs`

Owns bounded, control-character-free metadata projections used for diagnostics:

- model/provider/generation identifiers;
- OpenRouter route selection;
- bounded route-attempt summaries;
- finish reasons; and
- safe metadata strings.

This module receives parsed JSON values but never returns unknown provider text.

### `gateway/streaming/mod.rs`

Owns the cancellable SSE network loop:

- initial-header inactivity timeout;
- inactivity timeout between response chunks;
- cancellation selection;
- transport-activity and first-model-output accounting;
- wire/content bounds;
- final stream validation;
- usage recording; and
- normalized result construction.

### `gateway/streaming/sse.rs`

Owns the pure incremental parser and accumulator:

- fragmented CRLF/LF lines;
- SSE comments/heartbeats;
- `[DONE]`;
- content deltas and compatible final full-message snapshots;
- reasoning byte counts without retaining reasoning text;
- fragmented strict-tool arguments;
- finish reason;
- in-band errors;
- terminal usage; and
- content/wire limits.

It has no `reqwest`, Tokio, logging, cancellation registry, or database dependency.

### `gateway/streaming/usage.rs`

Owns safe usage normalization:

- OpenAI input/prompt/output/completion/total aliases;
- cached and cache-write counters;
- reasoning-token counters;
- nonnegative cost metadata;
- llama.cpp native timing/token aliases; and
- conversion to the task-local usage delta.

No source or generated content enters the usage model.

## 2. Profiles package

The root `ai/profiles.rs` file will be removed and replaced with:

```text
src-tauri/src/ai/profiles/
├── mod.rs
├── contracts.rs
├── commands.rs
├── service.rs
├── repository.rs
├── policy.rs
├── projections.rs
├── defaults.rs
└── tests/
    ├── mod.rs
    ├── contracts.rs
    ├── policy.rs
    ├── repository.rs
    ├── service.rs
    └── projections.rs
```

### `profiles/mod.rs`

Re-exports all current command functions and DTOs so `lib.rs`, providers, probes,
and document intelligence can continue using `ai::profiles::...` paths. It also
exposes narrow crate-private read models required by AI consumers.

### `profiles/contracts.rs`

Owns:

- all current create/update/delete/credential/default inputs;
- summary/detail/default/delete-result DTOs;
- `ProfileConfigurationStatus`;
- `ProfileSnapshot`; and
- a crate-private request snapshot containing the profile plus credential-storage
  mode.

Serde names and unknown-field policy remain unchanged. Provider kind remains
immutable.

### `profiles/commands.rs`

Contains only the Tauri boundary. Each command unwraps `State`, validates the
minimal identifier shape needed at IPC entry, and delegates to an ordinary Rust
service. It contains no SQL, endpoint comparison, secret-store operation, DTO
projection, or transaction.

Command names remain exactly:

```text
get_ai_provider_catalog
list_ai_provider_profiles
get_ai_provider_profile
create_ai_provider_profile
update_ai_provider_profile
delete_ai_provider_profile
set_ai_provider_credential
clear_ai_provider_credential
get_ai_task_defaults
set_ai_task_default
```

### `profiles/service.rs`

Owns application orchestration:

- the shared profile-mutation lane;
- operation replay decisions;
- current-revision checks;
- policy invocation;
- secret-store set/clear/status coordination;
- repository transaction calls;
- post-commit reload/projection; and
- ordinary read services for list/detail/request snapshots.

No network request occurs while holding the mutation lock or a SQLite
connection. Credential bytes never enter repository inputs or errors. The
existing order of secret-store and SQLite operations is initially preserved and
characterized; any compensation redesign for cross-resource failure requires a
separate reviewed behavior change rather than being hidden in a file move.

### `profiles/repository.rs`

Owns all profile aggregate SQL and row decoding:

- `ai_provider_profiles` list/load/create/update/delete;
- `ai_profile_operations` replay/record;
- profile capacity and normalized-name uniqueness;
- credential epoch/storage metadata updates;
- editor-draft commit participation;
- transactional invalidation of derived probe/capability evidence after relevant
  profile changes; and
- low-level request-snapshot loading.

Repository functions accept validated values and a connection/transaction. They
do not contact the secret store, validate URLs, create Tauri DTOs, or acquire the
mutation lock.

### `profiles/policy.rs`

Owns pure profile policy:

- provider implementation/auth compatibility;
- OpenRouter-only zero-data-retention configuration;
- display-name normalization and bounds;
- model-ID normalization and bounds;
- request-timeout bounds;
- profile/reference/operation identifier validation;
- optimistic revision validation; and
- credential removal requirements before endpoint-origin or authentication
  trust-boundary changes.

Generic URL parsing, local/LAN/cloud classification, fixed official endpoints,
and HTTP/HTTPS policy remain in `ai/endpoint_policy.rs`. `profiles/policy.rs`
consumes that stable endpoint service rather than duplicating it.

### `profiles/projections.rs`

Owns summary/detail construction and configuration-state calculation. It combines
stored profile metadata, validated endpoint scope, provider catalog labels, and
write-only credential status without exposing credential bytes.

### `profiles/defaults.rs`

Owns the task-default use case and its repository operations:

- read the current flashcard-generation default;
- validate an assigned profile exists and is implemented;
- persist/clear the default idempotently; and
- expose the crate-private default-profile request snapshot consumed by document
  intelligence.

This removes direct `ai_task_defaults` and credential-storage SQL from
`document_intelligence/context.rs`.

## 3. Probes package

The root `ai/probes.rs` file will be removed and replaced with:

```text
src-tauri/src/ai/probes/
├── mod.rs
├── contracts.rs
├── commands.rs
├── runner.rs
├── phases.rs
├── capabilities.rs
├── repository.rs
└── tests/
    ├── mod.rs
    ├── runner.rs
    ├── phases.rs
    ├── capabilities.rs
    └── repository.rs
```

### `probes/mod.rs`

Re-exports the three current Tauri commands and all serialized probe/model DTOs.
It also exposes narrow crate-private evidence readers used by document services.

### `probes/contracts.rs`

Owns:

- `AiProfileReferenceInput`;
- probe overall/phase statuses;
- capability status/source values;
- phase, capability, model-list, and result DTOs;
- adapter/result schema version constants; and
- crate-private evidence structures used between runner, capabilities, and
  repository.

Serialized values and capability keys remain unchanged.

### `probes/commands.rs`

Contains only Tauri wrappers for:

```text
list_ai_provider_models
test_ai_provider_profile
get_ai_provider_probe_result
```

The wrappers delegate to ordinary runner/repository functions and do not execute
provider requests or SQL directly.

### `probes/runner.rs`

Owns the ordered probe workflow and terminal semantics:

1. load and revision-check the profile request snapshot;
2. validate the profile and resolve its credential;
3. discover models;
4. verify the selected model, including the existing sole-model llama.cpp
   compatibility behavior;
5. read optional llama.cpp properties;
6. run the minimal text probe;
7. run the structured-output probe;
8. run the OpenRouter document-generation schema probe when applicable;
9. infer capabilities;
10. determine verified/partial/failed status; and
11. persist only if profile revision, credential epoch, and model still match.

Network work occurs without a SQLite connection or transaction. The runner
contains orchestration, not SQL or capability-key mutation details.

### `probes/phases.rs`

Owns phase-specific execution and phase DTO construction:

- profile validation;
- credential resolution evidence;
- model discovery;
- selected-model verification;
- optional server properties;
- text completion;
- structured output; and
- document schema/tool contract.

Each function returns explicit evidence to the runner. Optional failures remain
partial; required failures remain terminal. Phase labels/messages and latency
semantics remain stable.

### `probes/capabilities.rs`

Owns pure evidence-to-capability projection:

- default capability inventory;
- provider-specific supported/unsupported/unknown rules;
- declared versus probed source;
- context window;
- reasoning control;
- server chat-template presence/fingerprint;
- image input;
- usage reporting;
- JSON object/schema support; and
- document-generation schema support.

The logic is table-driven and independently testable. It does not read or write
SQLite and does not call providers.

### `probes/repository.rs`

Owns all durable probe/capability evidence:

- selected-model declaration persistence;
- probe-result load and bounded JSON decode;
- fresh-result conditional persistence;
- atomic replacement of result plus capability rows;
- freshness comparison against profile revision, credential epoch, and model;
- typed capability-record loading; and
- a compact document-probe evidence projection.

`document_provider.rs` and `document_intelligence/context.rs` will consume these
read functions instead of querying `ai_provider_capabilities` and
`ai_provider_probe_results` directly.

## 4. Other modules that must be updated

### `src-tauri/src/ai/mod.rs`

- Keep the existing `gateway`, `profiles`, and `probes` module names.
- Continue constructing one `AiGateway` in `AiRuntimeState`.
- Keep the secret manager and shared profile mutation lane.
- Adjust only imports/types needed by the new facades; do not broaden runtime
  visibility.

### `src-tauri/src/ai/providers.rs`

- Continue consuming the stable re-exported `AiGateway`, `AiGatewayError`, safe
  error constructor, and `ProfileSnapshot`.
- Preserve every provider route, request envelope, model parser, completion
  parser, and probe contract.
- No provider adapter is moved into the gateway.

### `src-tauri/src/ai/providers/openrouter.rs`

- Receive the document-specific OpenRouter request-policy helper currently hidden
  in the generic gateway: context compression disabled, no sticky session, and
  reasoning output excluded when reasoning is enabled.
- Preserve PR #72's provider routing and schema behavior byte-for-byte where
  semantically relevant.

### `src-tauri/src/ai/document_intelligence/transport.rs`

- Call the explicit document gateway method rather than generic `post_json`.
- Continue owning provider-specific document request bodies, schema lowering,
  llama.cpp compatibility retry, response decoding, and educational output
  validation.
- Preserve existing usage projection and task-specific errors.

### `src-tauri/src/ai/document_intelligence/context.rs`

- Replace direct `ai_task_defaults` and profile credential-storage queries with
  the profile/default read service.
- Replace direct probe-result/capability queries with the typed probe evidence
  read model.
- Preserve OpenRouter document-contract verification and image-input eligibility.

### `src-tauri/src/ai/document_provider.rs`

- Stop reading `ai_provider_capabilities` directly.
- Consume typed persisted capability records from `probes::repository` and retain
  the current provider-neutral planning model, structured-output strategy,
  reasoning resolution, and fingerprint.

### `src-tauri/src/ai/provider_contract_tests.rs`

- Update imports through stable facades.
- Retain the existing deterministic local mock-server coverage for llama.cpp,
  DeepSeek, OpenRouter, redirects, and semantic structured-output validation.
- Add explicit ordinary-versus-document transport characterization where the
  package unit tests do not cover the complete HTTP exchange.

### `src-tauri/src/ai/backup_tests.rs`

- Keep the current backup credential-sentinel test.
- Update imports only if internal profile/secret test helpers move.

### `src-tauri/src/lib.rs`

The preferred implementation keeps every existing
`ai::profiles::...`/`ai::probes::...` command path through facade re-exports, so
no command registration edit is expected. The IPC parity gate must prove the
native command surface is unchanged.

### Frontend AI modules

No production frontend change is expected:

- `src/ipc/aiProfiles.ts`
- `src/controllers/aiProfilesController.ts`
- `src/screens/settings/AiLlmSettings.tsx`

Their existing tests remain compatibility oracles. If a Rust move requires a
frontend type or command change, that is evidence the refactor crossed its
accepted boundary and must be corrected rather than propagated.

## 5. Static architecture enforcement

Add:

```text
scripts/verify-ai-infrastructure-boundaries.mjs
tests/scripts/test_ai_infrastructure_boundaries.py
```

Update:

```text
scripts/check-static.sh
scripts/verify-repository-policy.mjs
```

The verifier will enforce:

1. physical `gateway/`, `profiles/`, and `probes/` packages and normal module
   declarations;
2. absence of retired `gateway.rs`, `profiles.rs`, and `probes.rs` root files;
3. no path-attributed aliases or transitional facade directories;
4. a 48 KiB maximum per production Rust module as an anti-monolith tripwire;
5. `reqwest` confined to `src-tauri/src/ai/gateway/`, with client construction
   confined to `client.rs`;
6. no Tauri `State` or `#[tauri::command]` outside profile/probe `commands.rs`;
7. no SQLite/rusqlite dependency in the gateway;
8. profile aggregate table SQL confined to profile repositories/defaults,
   migrations, and explicit tests;
9. probe/capability insertion/select SQL confined to the probe repository, with
   the profile repository allowed only transactional invalidation deletes;
10. no gateway dependency on profiles, probes, provider adapters, documents,
    authoring, or Tauri;
11. no profile dependency on gateway/providers/probe runner; and
12. stable facade re-export markers for the current command and contract surface.

`verify-repository-policy.mjs` will change its single-file `reqwest` exception to
the gateway package and continue forbidding frontend networking and generic AI
IPC.

## 6. Test plan

### Characterization before movement

Before deleting any monolith, preserve or add tests for all behavior that could
be accidentally changed by moving code.

#### Gateway characterization

- fragmented SSE JSON across arbitrary byte chunks and CRLF/LF boundaries;
- comments/heartbeats count as transport activity but not model output;
- reasoning deltas count as progress but are never retained;
- content deltas plus a final full-message snapshot do not duplicate content;
- fragmented tool arguments and stable tool-name enforcement;
- terminal usage after content and before/after `[DONE]`;
- OpenAI usage aliases and llama.cpp timing normalization;
- malformed stream events and error events after partial output discard all
  partial output;
- ordinary JSON malformed response, redirect, HTTP error, and response-size
  bounds;
- fixed OpenRouter host attribution and redacted route metadata;
- cancellation while waiting for headers, during retry backoff, and during an
  active stream;
- exactly one retry for transient document failures and no retry for
  authentication, context length, invalid request, content policy, or completed
  structured-output truncation; and
- provider bodies containing secret-like sentinel text never appear in safe
  errors or routine diagnostics.

#### Profile characterization

- exact serde field names and unknown-field rejection for every input/DTO;
- provider/auth/privacy policy matrix;
- endpoint-origin/auth trust-boundary credential-removal rules;
- display-name/model/timeout normalization;
- create/update/delete optimistic revision behavior;
- operation replay and operation-ID conflict behavior;
- profile capacity and active-name uniqueness;
- editor-draft commit atomicity;
- credential epoch/storage metadata and probe invalidation;
- task-default assignment/clearing and deletion cleanup;
- summary/detail/configuration projections for missing, secure, session, disabled,
  incomplete, and planned states; and
- credential sentinel absence from SQLite and backups.

#### Probe characterization

- fixed phase order and stable required/optional terminal semantics;
- exact llama.cpp sole-model mismatch behavior and hard mismatch for cloud
  providers;
- table-driven capability inference across llama.cpp, DeepSeek, and OpenRouter;
- declared context/reasoning/server-template/image evidence;
- OpenRouter document-schema requirement;
- verified/partial/failed overall status;
- stale-result rejection when revision, credential epoch, or model changes;
- atomic probe-result/capability replacement;
- persisted result size limit and redaction; and
- typed document-probe/capability read projections used by document planning.

### Existing suites that must stay green

- `src-tauri/src/ai/provider_contract_tests.rs`
- `src-tauri/src/ai/backup_tests.rs`
- document-intelligence context/transport/provider tests
- `src/ipc/aiProfiles.test.ts`
- `src/controllers/aiProfilesController.test.tsx`
- `src/screens/settings/AiLlmSettings.test.tsx`
- `src/screens/SettingsScreen.test.tsx`
- `src/screens/SettingsScreen.extended.test.tsx`
- IPC contract verification
- repository policy and script tests
- full Rust, React, Linux/Tauri, Android package, and API-36 emulator gates

Normal tests use local fixtures and local mock servers. Real provider credentials
are never required by PR qualification.

## 7. Documentation updates during implementation

The planning ADR is this file. The implementation must also update:

- `docs/architecture/ai-and-document-processing.md` with the gateway/profile/probe
  package topology, explicit ordinary/document transport modes, and persistence
  ownership;
- `docs/architecture/document-ai-routing.md` with the explicit document gateway
  call and typed profile/probe read models;
- `docs/architecture/overview.md` with the native AI dependency direction;
- `docs/agents/AI-PROVIDER-GUARDRAILS.md` so `reqwest` is confined to the gateway
  package rather than the retired root file and so command/repository ownership
  is explicit;
- `docs/development/testing-and-ci.md` with the new static verifier and focused
  reproduction command;
- `docs/development/dependency-audit.md` to update the gateway path without
  changing the dependency graph; and
- `docs/decisions/0046-modular-ai-infrastructure.md` from Proposed to Accepted,
  with final topology and qualification evidence.

User-facing AI provider documentation should not change because behavior and UX
are non-goals.

## 8. Implementation sequence

The work remains one child PR but should be committed and qualified in bounded
stages.

### Stage A — Characterization and enforcement scaffolding

1. Add missing gateway/profile/probe characterization tests against current code.
2. Add the architecture verifier in a transitional mode that accepts the current
   root files but contains fixtures for the final topology.
3. Run static and fast qualification before moving code.

### Stage B — Gateway package

1. Introduce gateway contracts and facade.
2. Move pure SSE/usage/metadata/error logic first.
3. Move ordinary response/request execution.
4. Add the explicit document request method and retry owner.
5. Move document-only OpenRouter body policy to the provider/document builder.
6. Update document transport and provider-contract tests.
7. Remove `gateway.rs` and switch the verifier to final mode.
8. Run focused gateway tests, static, and fast qualification.

### Stage C — Profiles package

1. Move contracts and pure policy/projections.
2. Move SQL and row decoding into the repository/defaults owners.
3. Add ordinary service functions and leave Tauri wrappers thin.
4. Expose the narrow profile/default request snapshot.
5. Update probes and document context to consume it.
6. Remove `profiles.rs` and enable final profile verifier rules.
7. Run profile, controller, IPC, static, and fast tests.

### Stage D — Probes package

1. Move contracts and pure capability projection.
2. Move persisted evidence into the probe repository.
3. Split ordered orchestration from phase execution.
4. Expose typed evidence/capability readers.
5. Update document context and document provider.
6. Remove `probes.rs` and enable final probe verifier rules.
7. Run probe, provider-contract, document-planning, static, and fast tests.

### Stage E — Documentation and authoritative qualification

1. Update maintained architecture/guardrail/testing documentation.
2. Run `./scripts/check-static.sh` and `./scripts/check-fast.sh` on the exact head.
3. Run authoritative `./scripts/check.sh`.
4. Run Android compile/package and API-36 emulator smoke qualification.
5. Confirm no migration, lockfile, frontend IPC, prompt, schema, or checkpoint
   change.

## 9. Child-branch and merge protocol

The branch was created directly from PR #72's exact current head
`457451a3dbe27fe965c60c13ce94671cf944f093`. Until PR #72 merges, this pull
request targets `agent/xberg-document-adapter`, so its diff contains only the AI
infrastructure plan/refactor.

After PR #72 is squash-merged:

1. fetch the new `main`;
2. verify the PR #72 squash contains the parent head's reviewed behavior;
3. rebase or rebuild the child commits onto that exact `main` head;
4. retarget the child PR to `main`;
5. reconcile the PR ledger's base/head SHAs; and
6. rerun exact-head qualification.

No implementation commit should be copied onto pre-PR-#72 `main`, because the AI
refactor must retain Xberg ingestion and PR #72's sparse-evidence provenance fix.

## Acceptance criteria

- **AC-01 — Gateway package:** physical bounded package, explicit ordinary versus
  document request modes, unchanged request/error/retry/SSE/usage behavior, and
  no retired root file.
- **AC-02 — Profile package:** physical bounded package, thin commands, centralized
  service/policy/repository/projection/default ownership, unchanged IPC and
  credential privacy, and no retired root file.
- **AC-03 — Probe package:** physical bounded package, ordered runner, independent
  phase/capability logic, centralized evidence repository, unchanged result and
  capability contracts, and no retired root file.
- **AC-04 — Consumer cleanup:** providers, document transport, document context,
  and document planning consume stable facades/read models and no longer
  duplicate profile/probe table SQL.
- **AC-05 — Architecture enforcement:** static and fixture tests prevent legacy
  monoliths, hidden Tauri/SQL/network ownership, path aliases, and replacement
  monoliths.
- **AC-06 — Compatibility:** no migration, dependency, IPC, provider, prompt,
  checkpoint, parser, learning-artifact, backup, or frontend behavior change.
- **AC-07 — Qualification:** characterization, Rust, React, static, Linux/Tauri,
  Android package, and emulator gates pass on the exact final head.

## Risks and mitigations

### Accidental behavior changes during movement

Mitigation: add characterization tests first, move one responsibility at a time,
and qualify each package before starting the next.

### Circular profile/probe ownership

Mitigation: probes consume profile read models; profiles do not consume the probe
runner. Profile repository code may only delete derived evidence as part of a
source-profile mutation transaction.

### Over-generalized framework abstractions

Mitigation: use concrete functions and small structs for the three implemented
providers. Do not introduce a dependency-injection framework, dynamic plugin
registry, generic prompt client, or speculative provider trait hierarchy.

### Gateway/provider policy leakage

Mitigation: gateway owns transport mechanics and safe transport errors; provider
and document transport own provider request bodies and educational output
contracts. Static checks enforce the direction.

### Child branch drift while PR #72 merges

Mitigation: retain the exact parent SHA in this ADR and PR ledger, retarget only
after the parent squash is verified in `main`, and rerun exact-head checks.

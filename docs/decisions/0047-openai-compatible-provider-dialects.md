# ADR 0047: One OpenAI-compatible provider family with narrow dialects

- **Status:** Accepted
- **Date:** 2026-08-24
- **Pull request:** #73; DeepSeek compatibility clarification in #101

## Context

After splitting the AI gateway, profile aggregate, and probe subsystem, the next
source of accidental complexity was the provider layer itself. llama.cpp,
DeepSeek, and OpenRouter had evolved as three separate-looking integrations even
though their active ostudy paths share most of the same protocol:

- OpenAI-style model-list objects;
- Chat Completions `messages`, `choices`, finish reasons, and usage;
- JSON/SSE transport through the same native gateway;
- local Rust validation after structured generation; and
- the same profile, credential, probe, cancellation, and document-task services.

The meaningful differences are much narrower than three complete adapters.
Keeping common request bodies, response parsing, usage decoding, and document
orchestration duplicated makes drift likely. In particular, DeepSeek and
OpenRouter independently lowered the same strict-schema subset, document usage
had a second normalizer outside the gateway, and only OpenRouter was exercising
the representative document schema during profile verification.

## Decision

Treat all currently implemented providers as one **OpenAI-compatible protocol
family** and isolate only irreducible wire differences as dialect modules under
`src-tauri/src/ai/providers/`.

```text
providers/
├── mod.rs                 shared public protocol types/facade
├── dialect.rs             single provider dispatch seam
├── models.rs              common model discovery/projection
├── probes.rs              common text/structured/document probes
├── document.rs            common document request preparation/decoding
├── openai_compatible.rs   common Chat Completions body/response helpers
├── schema.rs              shared portable strict-schema lowering
├── llama_cpp.rs           llama.cpp route/properties/schema compatibility
├── deepseek.rs            DeepSeek thinking + strict-tool dialect
└── openrouter.rs          OpenRouter routing/privacy/catalog metadata
```

The gateway still owns HTTP, SSE, cancellation, retries, response bounds, safe
errors, and usage normalization. Provider modules do not own network clients or
persistence. Document intelligence owns prompts, source evidence, semantic
validation, and learning artifacts but no longer constructs provider wire
contracts.

We intentionally do **not** introduce a broad provider trait whose methods mirror
every current feature. `ProviderDialect` is an internal dispatch seam for the few
wire decisions that actually vary. Shared behavior remains ordinary functions
and data structures.

## Provider differences retained

### llama.cpp / Unsloth GGUF

Unsloth is a model/source workflow, not a separate API provider in ostudy. GGUF
models produced or distributed by Unsloth are served through llama.cpp and use
the llama.cpp dialect.

llama.cpp keeps custom code only for:

- accepting either a server-root or `/v1` base URL;
- optional `/props` metadata used for chat-template, vision, and context evidence;
- disabling model thinking through llama.cpp chat-template controls; and
- one structured-output compatibility fallback for server/version differences.

### DeepSeek

DeepSeek Chat Completions supports JSON-object output but does not expose the
same Chat Completions JSON-schema contract used by OpenRouter. Its beta strict
function-tool path also accepts only a restricted JSON-Schema subset.

For a document contract that fits that subset, ostudy uses the strict beta tool
path. Before sending the request, the provider adapter examines the lowered
schema rather than assuming every native contract is strict-tool compatible.
Contracts containing unsupported constructs such as nullable union types or
unsupported composition use DeepSeek's ordinary Chat Completions
`response_format: {"type":"json_object"}` path instead. The complete native
schema is still embedded in the structured-output instructions, and the same
bounded Rust parser/admission code remains authoritative after the response.

This distinction became necessary when interactive-question generation added
optional explanation/rationale fields represented as `type: ["string","null"]`.
Sending those schemas as strict tools caused an immediate HTTP 400 before model
inference. Falling back only at the provider wire boundary preserves those
legitimate native contracts instead of weakening validation or silently changing
the selected provider/model.

A future migration to DeepSeek's Responses API may permit a more uniform JSON
schema wire format, but that is a provider-behavior change and remains separate.

### OpenRouter

OpenRouter keeps custom code for behavior that is genuinely router-specific:

- `/models/user` with public-catalog fallback;
- structured-output support declarations and richer model metadata;
- provider routing, parameter compatibility, privacy/ZDR, precision, throughput,
  and latency preferences;
- disabled context compression for document work; and
- optional reasoning-effort routing while excluding returned reasoning content.

## Probe policy

The tiny connectivity structured-output probe and the representative document
contract are separate facts. Every implemented provider executes the exact
structured-output dialect used by real document generation against a
representative document schema. Provider-specific compatibility logic is tested
at this boundary: DeepSeek strict-tool-compatible schemas remain strict tools,
while schemas outside that subset use its JSON-object fallback.

This prevents both false positives (a tiny JSON object works but the real schema
does not) and false negatives (a provider's preferred strict route rejects a
native contract that its supported JSON-object route can still produce and Rust
can validate).

Persisted probe evidence is valid only when the profile revision, credential
epoch, model ID, **adapter revision**, and result-schema version all match the
current implementation. Changing provider-wire behavior therefore invalidates
old evidence without a database migration.

## Consequences

- Common request/response/usage behavior has one implementation.
- Adding another OpenAI-compatible provider should usually require only catalog
  policy plus a small dialect module.
- A provider that is not genuinely OpenAI-compatible can add a different
  protocol family later without distorting this abstraction.
- Provider-specific behavior is easier to audit because every exception lives in
  the provider dialect package instead of document or probe orchestration.
- Native schema/semantic admission remains authoritative even when a provider
  cannot enforce the complete schema on the wire.
- The static architecture verifier rejects provider-wire vocabulary returning to
  document transport and rejects provider persistence/Tauri ownership.

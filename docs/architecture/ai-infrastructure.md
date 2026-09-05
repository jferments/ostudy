# Native AI infrastructure

The [comprehensive architecture](overview.md) explains AI's place in ostudy. This reference defines the independently owned gateway, profile, probe, provider, and task boundaries. Optional AI never becomes a requirement for ordinary study, and a failed local provider never authorizes a cloud fallback.

## Ownership map

| Package | Owns | Must not absorb |
| --- | --- | --- |
| `ai/gateway/` | HTTP/SSE transport, common bounds, inactivity policy, retry classification, usage, safe errors | SQLite, profiles, cards, document lifecycle, UI |
| `ai/profiles/` | Profile contracts/policy/services/repository/projections and task defaults | Provider wire parsing or document prompts |
| `ai/probes/` | Ordered verification, typed capability evidence, freshness-stamped persistence | A second credential/profile aggregate |
| `ai/providers/` | Provider-family dialects, schemas/envelopes, model metadata, task transport dispatch | Direct storage, generated-card publication, screen-specific orchestration |
| `ai/document_intelligence/` | Bounded source-grounded task requests, schemas, response/domain interpretation | Raw HTTP or third-party parser models |
| `ai/secrets.rs` and platform store | Saved/session-only credential bytes | Learner-data columns or generic secret-read IPC |

`document_provider.rs` is a task-facing compatibility/projection boundary, not a second owner for credentials, models, or profile persistence. Inspect current callers before removing such a facade. [ADR 0046](../decisions/0046-modular-ai-infrastructure.md) and [provider dialect ADR](../decisions/0047-openai-compatible-provider-dialects.md) explain the decomposition.

## Gateway contract

The gateway owns `reqwest` and client construction. It rejects redirects and inappropriate endpoint schemes, does not use cookies as an application session, bounds response/stream data, normalizes usage, and translates provider failures into safe application errors. Retry policy is narrow and bounded; it is not permission to repeat arbitrary committed native operations or switch providers.

Transport captures a resolved request configuration. It does not fetch profile rows or discover task state while a request is running. Cancellation is supplied by the task owner. For streaming document work, the profile timeout measures inactivity between observable headers/events, not the total wall time of an entire generation job. Heartbeats, reasoning/output/tool deltas, and final usage can maintain liveness; silent transport eventually fails. Buffer limits still apply.

SSE framing/reassembly and provider payload interpretation are distinct layers. A successful HTTP code or a syntactically parsed event does not constitute a validated generated guide/card. The task layer must validate the final contract before publication.

## Profiles and secrets

Profiles contain provider family, nonsecret endpoint/model metadata, name, timeout, storage preference, reasoning/privacy policy, revision, and verification-related references. Commands delegate to a service that normalizes/validates policy, coordinates secrets, and calls a repository with explicit SQL. Read projections provide exactly the configuration/status a consumer needs.

Provider family is immutable. Built-in cloud endpoints are fixed. Changing endpoint/model/trust/credentials can invalidate prior verification and needs deliberate policy handling. Local loopback HTTP is distinct from explicitly acknowledged private-LAN HTTP; public cleartext HTTP is rejected. A profile's key must not silently be carried across a different trust boundary.

Saved keys live in the native platform secret service; session-only credentials are an explicit native-memory lifetime. **A newly typed key necessarily exists briefly in the frontend entry form and crosses the write boundary.** The prohibition is saved-secret readback, logging, and durable learner-data storage—not a false assertion that key entry never passes through frontend memory.

Secret-service mutation and SQLite metadata mutation are separate stores. The service owns their ordering, cleanup/recovery, credential epoch, and safe error semantics; a SQL transaction alone cannot make the OS secret service atomic. Generic profile DTOs expose saved-key availability/status, never the saved bytes. Secure-store unavailability must not block non-AI study.

Current profile reasoning/default semantics are in [AI model profiles](ai-model-profiles.md).

## Probes and evidence

A probe captures profile revision, credential epoch, endpoint/model identity, and requested tests. It performs external verification without keeping a library transaction/connection open across HTTP. Results are persisted only if the profile still matches the captured identity; stale success must not re-verify a model that changed during the request.

Verification includes appropriate endpoint/authentication, discovery/model selection, text, structured-output, and relevant image/context evidence. Capabilities distinguish **supported, unsupported, and unknown**, and declared metadata from actual probe evidence. Unknown is not a hidden successful capability. Representative task schemas matter: accepting a trivial JSON object does not necessarily prove a provider supports the schema shape document tasks use.

Probe phases and technical detail are available for diagnosis, while the ordinary Settings UI emphasizes the usable result. Connection-affecting edits invalidate old evidence. Tests use deterministic provider fixtures; they do not imply that every current live model/account has been exercised.

## Provider-family dispatch

Implemented adapters are **llama.cpp / Unsloth GGUF**, **DeepSeek**, and **OpenRouter**. Dedicated Ollama, OpenAI, Gemini, and Claude remain planned catalog entries. Filtering by the implementation flag is necessary; serializable enum variants are not implementation proof.

The compatible-dialect layer shares appropriate endpoint/model/message behavior without erasing differences in schema enforcement, strict tools, capabilities, reasoning effort, or response normalization. llama.cpp's server owns model loading and the tokenizer/chat template. DeepSeek document requests use the adapter's strict-function/schema conversion. OpenRouter requests enforce supported structured parameters and the profile's routing/privacy choices.

OpenRouter document policy denies provider data collection and disables provider context compression; optional required zero data retention further restricts routing. These are explicit request constraints and may cause no eligible endpoint to exist. The application must report failure, not silently relax them. Request policy is not an independent audit of the external service.

## Task boundary and publication

React requests narrow operations such as document generation, profile testing, or model discovery. It can receive nonsecret endpoint/model configuration and validated **domain results**. It cannot request an arbitrary completion proxy, execute provider-supplied commands, or retrieve saved keys.

Document intelligence consumes canonical source evidence and bounded windows, delegates wire operations to provider adapters/gateway, then checks response shape and domain semantics. Native authoring/guide services own final transactions, duplicates, stable topics, revisions, and managed assets. The provider cannot write the database merely by returning a well-formed object.

Prompts/schemas change with a versioned task contract. Checkpoints include identities sufficient to reject stale reuse. Local/LAN document request execution is serial under current policy; cloud work has a bounded concurrency ceiling and deterministic assembly/publication despite out-of-order network completion.

## Privacy must be scoped correctly

| Data | Intended destination |
| --- | --- |
| Source text, canonical derivatives, generated cards/guides, learner notes | Local domain storage; validated IPC/read models and coherent backups where the feature requires them |
| Nonsecret provider URL/model/profile/default metadata | Native metadata storage and appropriate Settings/task DTOs; backup or explicit sync scope |
| Saved credential bytes/private keys | Native secret/identity boundary, never generic learner-data exports or saved-key readback |
| Raw HTTP/SSE bodies, raw prompts/responses, reasoning buffers, authorization headers | Transient processing; excluded from ordinary diagnostics and generic IPC |
| Safe counts, engine/model identifiers, request outcomes, usage observations | Bounded redacted diagnostic/provenance records |

It is incorrect to say that generated text or source text never enters SQLite: storing useful learning material is the product. It is equally incorrect to log raw provider/source bodies merely because validated domain content is stored elsewhere. Channel, purpose, and validation boundaries matter.

Complete library backups include allowed nonsecret profile metadata, not saved keys. Ordinary sync excludes provider configuration; a sender can opt into the supported metadata/credential transfer for one authenticated encrypted offer through a separate secure-store path. Theme/application settings remain local during sync. See [sync architecture](device-sync.md).

## Enforcement and test surfaces

The AI infrastructure verifier scans ownership/import boundaries, `reqwest`, command registration, repository SQL, provider dialects, and audited compatibility owners. Provider/dialect, gateway transport/retry/streaming/redaction, profile revision/secret coordination, probe staleness, and document-task contract tests protect behavior. No verifier is permission to weaken the source boundary to make a refactor compile.

Use [AI infrastructure testing](../development/ai-infrastructure-testing.md), [provider contracts](../development/ai-provider-contract-testing.md), and [document AI routing](document-ai-routing.md). Live endpoints, real keys, account retention policy, and physical-device networking remain explicit manual acceptance unless actually exercised.

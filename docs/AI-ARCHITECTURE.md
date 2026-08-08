# ostudy AI architecture

## Purpose

ostudy's AI platform connects study features to model servers and cloud APIs that the user already operates or explicitly configures. The application does not install model weights, Python, CUDA, llama.cpp, Ollama, or any other model-serving infrastructure.

The architectural goal is to make future document ingestion, flashcard generation, tutoring, quizzes, and learning analytics share one secure native provider boundary rather than embedding provider logic in individual screens.

## Layering

The Stage 4.1/4.2 path follows the same architecture established during Stage 3:

```text
React Settings / future feature views
  -> frontend AI profile controller
  -> typed AI IPC wrappers
  -> narrow Tauri commands
  -> Rust AI profile / secret / probe services
  -> provider-neutral gateway
  -> provider adapter
  -> bounded native HTTP transport
```

React never owns provider networking or persisted credentials. `reqwest` is confined to the native AI subsystem, and the WebView CSP does not need provider network access.

There is intentionally no generic `send_ai_prompt` IPC command. Future features call task-specific native workflows so prompt construction, grounding, output schemas, local validation, and commit authority remain controlled by ostudy rather than arbitrary frontend messages.

## Provider profiles

A provider profile is a durable, nonsecret description of one trust boundary. It contains the provider kind, normalized endpoint, selected API model ID, authentication mode, timeout, enablement state, revision, and credential epoch.

Credentials are not part of profile metadata. They are stored in the operating-system credential store or, when explicitly selected, held only in native process memory for the current session. SQLite records only nonsecret credential-storage intent (`none`, `secure`, or `session`) so the runtime knows which secret boundary is authoritative. Credentials are excluded from backups, diagnostics, operation records, logs, and frontend read DTOs.

A session-only profile never falls through to an old secure credential after restart.

## Setup order and automatic discovery

Settings follows the dependency order required by real provider APIs:

1. **Provider connection** — create/save the provider identity and trusted endpoint.
2. **API credential** — submit a credential when the provider requires authentication.
3. **Available model** — as soon as the saved endpoint and credential state are sufficient, ostudy automatically performs native provider model discovery.
4. **Model selection** — the dropdown contains only model IDs returned by the current provider response. The user saves the desired API routing ID.
5. **Connection and capabilities** — verification runs against the saved endpoint, credential, and selected model.

Nothing that the provider can report reliably should require manual transcription. For a no-auth llama.cpp profile, model discovery can run immediately after the endpoint is saved. For authenticated providers such as DeepSeek, discovery waits until the API credential has crossed the native secret boundary. Opening an already-configured profile refreshes its model list automatically when credential state permits the request. A manual refresh remains available for retry/update, but is not the primary setup mechanism.

A stale saved model that is not present in the latest provider response is shown as stale configuration; it is not inserted into the current provider-backed dropdown.

## Initial provider adapters

### llama.cpp / Unsloth GGUF

Unsloth describes how a model was trained/exported; the runtime protocol is the llama.cpp server API. ostudy therefore configures these models as llama.cpp provider profiles.

For a normal llama.cpp server, ostudy uses the OpenAI-compatible routes:

- `GET /v1/models` for the loaded API model identity;
- `POST /v1/chat/completions` for text and structured-output probes.

A saved base URL may be the server root or may end in `/v1/`; adapter route construction normalizes both without producing a duplicate `/v1/v1/...` path.

llama.cpp may return the GGUF file path as the model ID unless the server was started with an explicit `--alias`. The API-reported ID is the routing identity ostudy should save and send back to that server.

`GET /props?model=<id>` is optional metadata. If it is absent or returns non-JSON through a proxy/router, that does not invalidate otherwise successful model discovery, text generation, or schema-constrained output. When properties are available, ostudy records only bounded capability metadata and a hash of the server/model chat template; it never persists the template itself.

The server/model remains responsible for tokenizer-level chat formatting. ostudy does not maintain hand-written Jinja/ChatML templates.

### DeepSeek

DeepSeek uses its fixed official HTTPS API endpoint and bearer authentication. Model discovery uses the provider's `/models` endpoint after the API key has been saved.

Connectivity probes intentionally disable thinking and streaming so a tiny test verifies final-answer generation rather than consuming its bounded output budget on reasoning. This is a connectivity policy only; later learning tasks may explicitly enable reasoning when appropriate.

DeepSeek structured output uses JSON Object mode. ostudy then applies strict local Rust deserialization and semantic validation. Provider-side JSON Schema is therefore represented as **not offered**, while **Local validation** records the guarantee ostudy actually enforces.

## Native gateway and transport

The gateway owns:

- endpoint normalization and Local/LAN/Cloud policy;
- bearer attachment only after endpoint validation;
- bounded request and response sizes;
- connect/request timeouts;
- disabled redirects;
- Rustls TLS;
- no cookie store;
- no raw request/response body logging;
- provider-specific error normalization.

The gateway returns normalized safe errors rather than arbitrary provider bodies. This lets UI and future task services reason about authentication, rate limits, missing models, timeouts, protocol errors, malformed structured output, and unsupported capabilities without exposing secret or model content.

## Capability probes

A profile test is explicit; it never runs automatically on app startup or merely because a credential was entered. The probe checks:

1. saved profile validity;
2. credential availability;
3. provider model discovery;
4. selected-model availability;
5. optional server/model metadata where supported;
6. minimal text completion;
7. minimal structured output;
8. local structural and semantic validation;
9. normalized capability synthesis.

Capability evidence distinguishes `declared` from `probed` and `supported`, `unsupported`, and `unknown`.

Probe results are bound to the profile revision, credential epoch, model ID, adapter revision, and result-schema version. If configuration changes during an in-flight request, the result cannot replace current capability evidence.

Persisted probe records contain no raw prompt, response, reasoning, credential, Authorization header, or complete chat template.

## Plaintext connection diagnostics

After Test connection, Settings renders a selectable plaintext **Connection diagnostic log**. It records safe operational facts such as provider, normalized endpoint, destination scope, configured/tested model ID, attempted routes, adapter/result versions, phase status, latency, normalized error codes, and a bounded model-list preview.

For llama.cpp diagnostics, the required OpenAI-compatible discovery route is shown as `/v1/models`; `/props` is explicitly identified as optional metadata.

The transcript deliberately excludes credentials, Authorization headers, raw prompts, raw provider responses, reasoning content, and full chat templates. It is intended to be safe to paste into a bug report.

## No fallback and no provider authority over data

There is no automatic provider fallback. A failed local profile will never silently cause study content to be sent to a cloud provider.

Provider adapters also have no direct authority to create, edit, or delete cards. Future AI generation flows must produce validated candidate data that is reviewed/committed through canonical authoring services.

## Extension rules

Adding Ollama, OpenAI, Gemini, Claude, or another provider requires:

- a catalog entry and explicit adapter;
- endpoint/authentication policy;
- model-discovery behavior where the provider exposes it;
- provider-specific structured-output lowering;
- deterministic contract fixtures;
- local output validation;
- safe normalized errors and capability evidence;
- UI driven by provider metadata rather than provider-specific network calls.

Provider name alone must never be treated as proof of a capability. Capability data is declared or probed and remains versioned/redacted.

## Later document and task services

Stages after 4.2 can build on this provider foundation with managed immutable documents, PDF extraction/rendering, FTS retrieval, source-evidence references, durable AI jobs, candidate review, and task-specific schemas. Those layers should not weaken the provider/secret boundaries established here.

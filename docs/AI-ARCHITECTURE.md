# ostudy AI architecture

## Purpose

ostudy's AI platform connects study features to model servers and cloud APIs that the user already operates or explicitly configures. The application does not install model weights, Python, CUDA, llama.cpp, Ollama, or any other model-serving infrastructure.

Document import is local-first. Source PDFs are copied into managed application storage, parsed locally, and rendered locally before any provider request. A provider receives only the bounded page text and, when supported by the selected model, the rendered page image needed for the explicit flashcard-generation task.

## Layering

The AI and document path follows the same native-boundary architecture established during Stage 3:

```text
React Settings / Documents screens
  -> typed frontend IPC wrappers
  -> narrow Tauri commands
  -> Rust document and AI task services
  -> provider-neutral gateway
  -> provider adapter
  -> bounded native HTTP transport
  -> canonical Rust card authoring
```

React never owns provider networking, persisted credentials, PDF bytes, rendered page images, or provider response parsing. `reqwest` is confined to the native AI subsystem, and the WebView CSP does not need provider network access.

There is intentionally no generic `send_ai_prompt` IPC command. Each feature calls a task-specific native workflow that owns prompt construction, source grounding, output schemas, validation, and commit authority.

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

Nothing that the provider can report reliably should require manual transcription. For a no-auth llama.cpp profile, model discovery can run immediately after the endpoint is saved. For authenticated providers such as DeepSeek, discovery waits until the API credential has crossed the native secret boundary. Opening an already-configured profile refreshes its model list automatically when credential state permits it. A manual refresh remains available for retry/update.

A stale saved model that is not present in the latest provider response is shown as stale configuration; it is not inserted into the current provider-backed dropdown.

## Initial provider adapters

### llama.cpp / Unsloth GGUF

Unsloth describes how a model was trained/exported; the runtime protocol is the llama.cpp server API. ostudy therefore configures these models as llama.cpp provider profiles.

For a normal single-model llama.cpp server, ostudy uses the OpenAI-compatible routes:

- `GET /v1/models` for the loaded API model identity;
- `POST /v1/chat/completions` for text, structured-output, and document-flashcard requests.

A saved base URL may be the server root or may end in `/v1/`; adapter route construction normalizes both without producing a duplicate `/v1/v1/...` path.

Bare `GET /models` is treated as the separate llama.cpp router catalog rather than a universal single-model route. `GET /props?model=<id>` remains optional metadata. A missing or non-JSON properties response does not invalidate model discovery, text generation, or structured output.

The server/model remains responsible for tokenizer-level chat formatting. ostudy does not maintain hand-written Jinja/ChatML templates. When llama.cpp declares image input and the latest saved probe evidence supports it, document generation includes one locally rendered PNG page alongside that page's extracted text. Otherwise the task uses extracted text only; it never silently switches providers.

### DeepSeek

DeepSeek uses its fixed official HTTPS API endpoint and bearer authentication. Model discovery uses the provider's `/models` endpoint after the API key has been saved.

DeepSeek structured output uses JSON Object mode. ostudy then applies strict local Rust deserialization and semantic validation. Provider-side JSON Schema is represented as **not offered**, while **Local validation** records the guarantee ostudy actually enforces.

The current document task uses extracted page text with DeepSeek. It does not pretend that a text-only profile accepted the rendered page image.

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

The gateway returns normalized safe errors rather than arbitrary provider bodies. This lets UI and task services reason about authentication, rate limits, missing models, timeouts, protocol errors, malformed structured output, and unsupported capabilities without exposing secret or model content.

## Capability evidence

A profile test is explicit; it never runs automatically on app startup or merely because a credential was entered. Capability evidence distinguishes `declared` from `probed` and `supported`, `unsupported`, and `unknown`.

Probe results are bound to the profile revision, credential epoch, model ID, adapter revision, and result-schema version. If configuration changes during an in-flight request, the result cannot replace current capability evidence.

Persisted probe records contain no raw prompt, response, reasoning, credential, Authorization header, or complete chat template.

## Managed PDF document workflow

The Documents section implements one direct workflow:

```text
select PDF
  -> validate signature, name, and size
  -> copy immutable source into managed document storage
  -> extract bounded text from every page locally
  -> render every page to a bounded PNG locally
  -> send one page at a time to the selected flashcard-generation profile
  -> parse and validate strict Basic/Cloze card JSON
  -> create a new deck through canonical authoring
```

The model never receives the original PDF file, a local path, database access, or filesystem authority. Document text is embedded as untrusted source data and cannot replace the task prompt or output contract.

Generation is page-bounded. One page failure stops the task with a durable, safe error and leaves the managed PDF available for retry. Durable job rows record provider/model identity, progress, safe status, and card counts; they do not retain raw provider responses, model reasoning, or duplicate copies of generated question and answer bodies.

There is deliberately no generated-card approval queue. Once every page response has passed the task-level schema checks, the complete generated deck is committed through the existing in-memory `.ostudy.json` authoring pipeline. That pipeline remains authoritative for restricted Markdown, cloze syntax, tags, duplicates, generated card directions, fresh schedules, and transaction rollback. Deck/card creation, document-to-deck provenance, and generation completion are committed in the same SQLite transaction. The resulting deck and cards are ordinary ostudy data and can be edited, moved, archived, or deleted through existing interfaces.

A retry reuses complete extracted text and rendered pages rather than repeating local PDF work. If the generated deck is later soft-deleted, the document projection treats that deck as absent and offers regeneration from the retained PDF.

Source PDFs, extracted page text, rendered page ledgers, generation state, and generated-deck provenance are durable SQLite/managed-file data. Full-library backup version 2 includes source PDFs and rendered pages while continuing to exclude provider credentials.

## Plaintext connection diagnostics

After Test connection, Settings renders a selectable plaintext **Connection diagnostic log**. It records safe operational facts such as provider, normalized endpoint, Local/LAN/Cloud scope, configured/tested model, attempted routes, adapter/result versions, phase status, latency, normalized error codes, and a bounded model-list preview.

The transcript deliberately excludes credentials, Authorization headers, raw prompts, raw provider responses, reasoning content, complete chat templates, PDF text, and rendered page data.

## No fallback and no provider authority over data

There is no automatic provider fallback. A failed local profile will never silently cause study content to be sent to a LAN or cloud provider.

Provider adapters have no direct database or filesystem authority. The document task obtains validated output through an adapter, then calls canonical Rust authoring. The model cannot request replacement, deletion, schedule preservation, shell access, arbitrary local paths, or database mutations.

## Extension rules

Adding Ollama, OpenAI, Gemini, Claude, or another provider requires:

- a catalog entry and explicit adapter;
- endpoint/authentication policy;
- model-discovery behavior where the provider exposes it;
- provider-specific text/image content lowering;
- provider-specific structured-output lowering;
- deterministic contract fixtures;
- local output validation;
- safe normalized errors and capability evidence;
- UI driven by provider metadata rather than provider-specific network calls.

Provider name alone must never be treated as proof of a capability. Capability data remains declared or probed, versioned, bounded, and redacted.

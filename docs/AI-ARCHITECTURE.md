# ostudy AI provider platform

## Scope

Stages 4.1 and 4.2 establish the configuration, credential, and provider-gateway
foundation used by later document-grounded flashcard generation, quizzes, and
interactive tutoring. These stages do not install, configure, or run model
servers. Users connect ostudy to an existing local server or a cloud API.

## Durable boundaries

```text
React Settings feature
        ↓
settings and AI-profile controllers
        ↓
typed IPC gateways
        ↓
Tauri command boundary
        ↓
Rust AI profile and gateway services
        ↓
SQLite metadata + native secret store + provider HTTP APIs
```

Provider traffic originates only in Rust. The WebView never receives persisted
credentials, Authorization headers, raw provider responses, or a generic
"send prompt" command. Future task services call the internal gateway through
narrow task-specific workflows.

## Provider profiles

AI provider configuration is separate from ordinary `AppSettings`. Each profile
has a stable identity, immutable provider kind, normalized base URL, selected
model, authentication mode, bounded timeout, enabled state, optimistic revision,
and a monotonically increasing credential epoch. Task defaults reference a
single profile explicitly; ostudy never silently falls back from a local profile
to a cloud provider.

Initial implemented providers are:

- llama.cpp, including GGUF models exported or fine-tuned with Unsloth;
- DeepSeek's official API.

Ollama, OpenAI, Gemini, and Claude remain catalogued as planned providers until
their native adapters are implemented.

## Credentials

Credential bytes never enter SQLite, backups, diagnostics, logs, operation
records, or frontend read state. The database stores only nonsecret profile
metadata, credential-storage intent (`none`, `secure`, or `session`), and
credential epochs. A Rust secret boundary resolves credentials from the native
operating-system keyring or from explicit session-only process memory. There is
no plaintext fallback.

A profile marked session-only never falls through to a stale secure credential
after restart. The frontend receives only credential status and cannot read a
saved value back through IPC.

## Endpoint policy

Only `http` and `https` endpoints are accepted. User information, fragments,
and query strings are rejected. Loopback HTTP is allowed; private-network HTTP
requires explicit acknowledgement; public cleartext HTTP is rejected. Built-in
cloud-provider endpoints are fixed so a cloud credential cannot be redirected
to an arbitrary host. Redirects are rejected by the native HTTP client.

## Gateway

The internal `AiGateway` normalizes provider requests, generation bounds,
finish reasons, usage metadata, and safe errors. Provider adapters own
wire-format differences. The initial output constraints are ordinary text, JSON
object mode, and JSON Schema mode.

llama.cpp remains responsible for applying the model's embedded chat template.
ostudy detects template presence for diagnostics but does not store, expose, or
normally replace the full template. The bounded connectivity probes request
non-thinking operation when supported so a tiny probe measures final-answer
behavior rather than consuming its output budget on reasoning.

DeepSeek V4 connectivity probes likewise explicitly disable thinking. Future
learning tasks may deliberately enable provider reasoning when their task policy
calls for it; connectivity verification is intentionally different because it
needs a small, deterministic final answer. DeepSeek structured probes use JSON
object mode followed by independent strict Rust and semantic validation.

A provider-reported `finish_reason` of `length` is diagnostic evidence, not an
automatic failure when the final content already satisfies the exact probe
contract. Invalid, empty, or semantically wrong final content still fails.

## llama.cpp model identity

llama.cpp's OpenAI-compatible model ID is a server-routing identity, not
necessarily a Hugging Face repository name. A single-model server commonly
reports the loaded GGUF file path unless it was started with an explicit alias.

When a saved llama.cpp profile does not match `/v1/models` but the server reports
exactly one loaded model, ostudy treats that as a recoverable identity mismatch:

- Test connection records a partial model-verification result;
- the remaining probes use the server-reported model ID for that test only;
- Refresh models places the server-reported ID into the editable draft;
- the user must explicitly save the profile before the new ID becomes durable.

Multiple-model mismatches and DeepSeek model mismatches remain hard failures.

## Capability evidence

Provider capabilities are not inferred solely from provider names. A connection
probe verifies endpoint reachability, authentication, selected-model identity,
text completion, and constrained structured output. Capability records
distinguish `supported`, `unsupported`, and `unknown`, and whether the value was
declared by the server or actively probed. Results are bound to the profile
revision, credential epoch, saved model, and adapter revision so stale probes
cannot become current after configuration changes.

## Privacy and diagnostics

Persisted probe records contain only bounded, typed, redacted data: profile and
model identity, phase status, safe error code, latency, capability evidence, and
safe usage metadata where returned. Prompts, provider response bodies, reasoning
content, chat-template text, Authorization headers, and credentials are never
persisted by the profile-verification path.

The Settings UI exposes **View diagnostic information** after a probe. It shows
only redacted operational metadata such as provider, endpoint, destination
scope, configured model, adapter/result-schema versions, refreshed model IDs,
phase status, latency, and safe error codes. It deliberately does not expose raw
request or response content.

## Future document-grounded tasks

Later flashcard and tutor services will reuse the same profile, secret, endpoint,
gateway, structured-output, and capability services. Documents and learning
history will be selected and validated by those task services; the gateway will
not itself receive database access or authority to create cards.

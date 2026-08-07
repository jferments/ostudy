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
records, or frontend state. The database stores only nonsecret profile metadata
and credential epochs. A Rust `SecretStore` abstraction resolves credentials
from a native operating-system keyring when available and supports an explicit
session-only in-memory credential when secure persistence is unavailable. There
is no plaintext fallback.

The frontend receives only a status such as `missing`, `savedSecurely`,
`sessionOnly`, `notRequired`, or `secureStoreUnavailable`. Saved credential
values cannot be read back through IPC.

## Endpoint policy

Only `http` and `https` endpoints are accepted. User information, fragments,
and query strings are rejected. Loopback HTTP is allowed; private-network HTTP
requires explicit acknowledgement; public cleartext HTTP is rejected. Built-in
cloud-provider endpoints are fixed so a cloud credential cannot be redirected
to an arbitrary host. Redirects are rejected by the native HTTP client.

## Gateway

The internal `AiGateway` normalizes text messages, model identity, output
constraints, generation bounds, finish reasons, usage metadata, and safe errors.
Provider adapters own wire-format differences. The initial output constraints
are ordinary text, JSON object mode, and JSON Schema mode.

llama.cpp remains responsible for applying the model's embedded chat template.
ostudy detects template presence for diagnostics but does not store, expose, or
normally override the full template. DeepSeek uses JSON object mode followed by
independent local schema and semantic validation.

## Capability evidence

Provider capabilities are not inferred solely from provider names. A connection
probe verifies endpoint reachability, authentication, selected-model
availability, text completion, and constrained structured output. Capability
records distinguish `supported`, `unsupported`, and `unknown`, and whether the
value was declared by the server or actively probed. Results are bound to the
profile revision, credential epoch, model, and adapter revision so stale probes
cannot become current after configuration changes.

## Privacy and diagnostics

Persisted probe records contain only bounded, typed, redacted data: provider
kind, model ID, phase status, safe error code, latency, finish reason, and usage
counts where returned. Prompts, provider response bodies, reasoning content,
chat-template text, and credentials are never persisted by default.

## Future document-grounded tasks

Later flashcard and tutor services will reuse the same profile, secret, endpoint,
gateway, structured-output, and capability services. Documents and learning
history will be selected and validated by those task services; the gateway will
not itself receive database access or authority to create cards.
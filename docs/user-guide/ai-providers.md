# AI providers

AI is optional in ostudy. The flashcard library, editor, practice scheduler, analytics, search, imports, and backups work without a model connection. An AI profile becomes relevant only when you ask the application to perform a supported model-assisted task, currently PDF-to-flashcard generation.

ostudy is a client of an existing model service. It does not install llama.cpp, Unsloth, Ollama, a GPU runtime, or a cloud account for you.

## Provider status

| Provider family | Current status | Notes |
| --- | --- | --- |
| llama.cpp / Unsloth GGUF | Available | Connects to an existing llama.cpp-compatible server and uses the server/model chat template. |
| DeepSeek | Available | Connects to DeepSeek's official cloud API using the configured credential. |
| OpenRouter | Available | Uses OpenRouter's fixed official API, account-visible model discovery, strict structured output, and privacy-preserving routing defaults. |
| Ollama | Planned | Listed in the catalog, but the native adapter is not implemented. |
| OpenAI | Planned | Listed in the catalog, but the native adapter is not implemented. |
| Gemini | Planned | Listed in the catalog, but the native adapter is not implemented. |
| Claude | Planned | Listed in the catalog, but the native adapter is not implemented. |

A provider appearing in the settings catalog is not the same as being available for tasks. The interface and documentation should preserve that distinction.

## What a profile contains

A provider profile records nonsecret configuration such as:

- a stable local profile name and provider family;
- the normalized endpoint URL;
- the model identifier;
- whether authentication is required;
- request timeout and enabled state;
- capability evidence from the most recent successful verification;
- whether the profile is the default for a supported task.

Credential bytes are not stored in the SQLite library. Secure credentials use the operating-system credential service when available. A deliberately selected session-only credential remains in native process memory and disappears when ostudy exits.

## Configure llama.cpp or an Unsloth GGUF server

Start and manage the model server separately. The default ostudy endpoint is:

```text
http://127.0.0.1:8080/
```

Enter the endpoint and exact model identifier in **Settings → AI/LLM**, then verify the profile. The server is responsible for loading the model and applying its tokenizer-specific chat template. ostudy does not guess or build a separate template in the WebView.

A loopback endpoint keeps provider traffic on the same computer. A LAN endpoint can place document context on another machine under your control, but it also creates a network trust decision: secure the server, bind it intentionally, restrict access, and do not expose an unauthenticated inference service to an untrusted network.

Large local jobs are divided into bounded source windows. The application does not require the complete document to fit into one model request, but each request still has to fit the context window reported or configured for the selected model.

## Configure DeepSeek

Create a DeepSeek API credential directly with that provider, select DeepSeek in ostudy, enter the model identifier and credential, and verify the profile. DeepSeek uses its fixed official API base URL; ostudy does not accept an arbitrary endpoint for that provider family.

Cloud requests can incur provider charges and transmit selected source context outside your computer. ostudy does not add a study-app subscription, but it cannot guarantee a particular API cost. Document size, generated-card volume, retries, model pricing, and provider behavior all matter. Review the provider's current pricing and data terms before sending sensitive material.

## Verification and capabilities

Profile verification checks the configured endpoint, authentication, model discovery where supported, and the capabilities needed by ostudy's narrow task contracts. Capability evidence is tied to the profile revision, credential epoch, and model. Changing any of them invalidates older evidence rather than assuming that a previous result still applies.

A successful HTTP response is not enough. For document generation the provider must also support the structured-output behavior expected by its adapter. Model responses are parsed into strict native types and validated again before any card is created.

## Endpoint and transport safety

Provider traffic is owned by the native Rust gateway, not by frontend JavaScript. The gateway:

- rejects redirects;
- uses bounded request and response bodies;
- stores no cookies;
- forbids public cleartext HTTP;
- normalizes errors before they reach the interface;
- does not log credentials, authorization headers, raw prompts, raw responses, or model reasoning content.

Loopback and intentionally configured local-network HTTP endpoints are treated differently from public internet endpoints because many local model servers do not provide TLS. A cloud provider must use HTTPS.

## No silent fallback

ostudy never changes providers because a request failed, a model was slow, or a local server was offline. Retrying a task uses the profile selected for that task unless you explicitly choose another profile. This prevents private source material intended for a local model from being sent to a cloud service unexpectedly.

## What is sent during PDF generation

The original PDF is stored locally and is not uploaded as an opaque file. ostudy performs local extraction, semantic organization, rendering, and OCR, then sends bounded text context and selected rendered pages when the verified provider/model supports image input. The exact amount depends on document structure, task windows, and provider capability.

The application records safe progress and usage measurements needed to resume or diagnose a job, not raw provider messages or document text in diagnostic exports.

## Troubleshooting

When verification fails, check these in order:

1. The model server or cloud account is actually available.
2. The endpoint includes the correct scheme, host, port, and base path.
3. The model identifier matches the provider's exact identifier.
4. The selected authentication mode matches the endpoint.
5. A credential is present in the secure or session-only store when required.
6. A firewall, proxy, VPN, or container boundary is not making `127.0.0.1` refer to the wrong machine.
7. The model supports the structured output and context size required by the task.

Use profile verification before starting a long document job. A failed profile test should be fixed at the provider boundary rather than retried repeatedly through document generation.

For implementation details, see [AI and document processing](../architecture/ai-and-document-processing.md).

## OpenRouter

Create an **OpenRouter** profile in Settings → AI/LLM, save the API key in the operating-system credential store, retrieve the account-visible model catalog, select a model, and run the connection/capability test. ostudy uses OpenRouter's fixed official API endpoint.

For document generation, ostudy requests strict JSON Schema output only from parameter-compatible endpoints and sets OpenRouter's provider-data-collection policy to `deny`. It does not enable provider-side prompt/body debugging or context-compression plugins. A stricter policy can reduce endpoint availability; the connection test reports that as a routing/capability failure rather than silently weakening the request.

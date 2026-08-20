# AI providers

AI is optional in ostudy. The flashcard library, editor, practice scheduler, analytics, search, imports, and backups work without a model connection. An AI profile becomes relevant only when you ask the application to perform a supported model-assisted task, currently PDF-to-flashcard generation.

ostudy is a client of an existing model service. It does not install llama.cpp, Unsloth, Ollama, a GPU runtime, or a cloud account for you.

## Provider status

| Provider family | Current status | Notes |
| --- | --- | --- |
| llama.cpp / Unsloth GGUF | Available | Connects to an existing llama.cpp-compatible server and uses the server/model chat template. |
| DeepSeek | Available | Connects to DeepSeek's official cloud API using the configured API key. |
| OpenRouter | Available | Uses OpenRouter's fixed official API, account-visible model discovery, strict structured output, and privacy-preserving routing defaults. |
| Ollama | Planned | Listed in the native catalog, but the adapter is not implemented. |
| OpenAI | Planned | Listed in the native catalog, but the adapter is not implemented. |
| Gemini | Planned | Listed in the native catalog, but the adapter is not implemented. |
| Claude | Planned | Listed in the native catalog, but the adapter is not implemented. |

Only providers implemented by the current build are offered by the normal **Add model** flow.

## Normal setup flow

Open **Settings → AI/LLM** and select **Add model**. The ordinary setup path deliberately asks only for information needed to establish a usable connection:

1. Choose the provider.
2. For a local llama.cpp-compatible service, enter the **Server URL**. For a cloud provider that requires authentication, enter the **API key**.
3. Select **Connect and load models**. ostudy saves the connection metadata, stores the API key according to the selected credential-storage policy, and asks the provider for its available models.
4. Choose a model from the returned list and select **Save profile**.
5. Select **Test connection** to verify the saved model and the capabilities needed by ostudy.

The setup screen does not expose transport-authentication jargon, a separate enable switch, or timeout policy in the normal path. Those are either fixed by the provider or kept under **Advanced settings** when they are genuinely configurable.

A saved profile can be selected as the **Default AI model** for flashcard generation. ostudy does not silently fall back to another model or provider if that profile fails.

## Advanced settings

Open **Advanced settings** only when you need to change an existing supported profile field. Depending on the provider, the disclosure can contain:

- the local profile name;
- the provider inactivity timeout;
- secure-device versus session-only API-key storage;
- optional API-key authentication for a llama.cpp-compatible server;
- acknowledgement for an unencrypted HTTP server elsewhere on a private LAN;
- **Require zero data retention** for OpenRouter;
- saved API-key removal and profile deletion.

Changing connection-affecting settings such as endpoint, authentication, LAN transport policy, OpenRouter retention policy, or API key requires reconnecting and loading the model catalog again. Changing ordinary metadata such as the profile name or inactivity timeout can be saved without pretending that the provider connection changed.

## What a profile contains

A provider profile records nonsecret configuration such as:

- a stable local profile name and provider family;
- the normalized endpoint URL;
- the selected model identifier;
- the provider authentication policy;
- the provider inactivity timeout;
- local-network transport acknowledgement where applicable;
- an optional OpenRouter-only zero-data-retention requirement;
- capability evidence from the most recent verification;
- whether the profile is the default for a supported task.

Credential bytes are not stored in the SQLite library. Secure credentials use the operating-system credential service when available. A deliberately selected session-only credential remains in native process memory and disappears when ostudy exits. The interface never reads a saved key back into the API-key field; enter a value only when adding or replacing a key.

## Configure llama.cpp or an Unsloth GGUF server

Start and manage the model server separately. The default ostudy endpoint is:

```text
http://127.0.0.1:8080/
```

Open **Settings → AI/LLM**, select **Add model**, leave **llama.cpp / Unsloth GGUF** selected, enter the server URL if the default is not correct, then choose **Connect and load models**. Select the model reported by the server, save the profile, and test the connection.

Most local servers do not require an API key. If yours does, open **Advanced settings**, change **Server authentication** to **API key**, enter the key in the normal API-key field, and reconnect. The server remains responsible for loading the model and applying its tokenizer-specific chat template; ostudy does not guess or build a separate template in the WebView.

A loopback endpoint keeps provider traffic on the same computer. A LAN endpoint can place document context on another machine under your control, but it also creates a network trust decision: secure the server, bind it intentionally, restrict access, and do not expose an unauthenticated inference service to an untrusted network. Unencrypted LAN HTTP requires the explicit acknowledgement under Advanced settings.

Large local jobs are divided into bounded source windows. The application does not require the complete document to fit into one model request, but each request still has to fit the context window supported by the selected model and provider.

## Configure DeepSeek

Create a DeepSeek API key directly with that provider. In ostudy, add a model, choose **DeepSeek**, enter the API key, and select **Connect and load models**. Choose one of the models returned by the account-visible catalog, save the profile, and test it. DeepSeek uses its fixed official API base URL; ostudy does not ask for or accept an arbitrary endpoint for that provider family.

Cloud requests can incur provider charges and transmit selected source context outside your computer. ostudy does not add a study-app subscription, but it cannot guarantee a particular API cost. Document size, generated-card volume, retries, model pricing, and provider behavior all matter. Review the provider's current pricing and data terms before sending sensitive material.

## Configure OpenRouter

Create an OpenRouter API key directly with that provider. In ostudy, add a model, choose **OpenRouter**, enter the API key, and select **Connect and load models**. Choose a model from the account-visible catalog, save the profile, and run the connection test. ostudy uses OpenRouter's fixed official API endpoint.

For document generation, ostudy requests strict JSON Schema output only from parameter-compatible endpoints and sets OpenRouter's provider-data-collection policy to `deny`. It explicitly disables provider-side context compression so document windows are not silently changed, and it never enables prompt/body debugging. Enable **Require zero data retention** under Advanced settings to send the stronger zero-retention routing requirement. That restriction can reduce endpoint availability, and the connection test reports the result instead of silently weakening the profile.

## Verification and capabilities

Profile verification checks the configured endpoint, authentication, model discovery where supported, and the capabilities needed by ostudy's narrow task contracts. Capability evidence is tied to the profile revision, credential epoch, and model. Changing any of them invalidates older evidence rather than assuming that a previous result still applies.

The settings screen presents the ordinary result first, such as **Connection verified**. Open **Technical details** when you need the individual probe phases and capability evidence.

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

The application records safe progress and usage measurements needed to resume or diagnose a job, not raw provider messages or document text in diagnostic exports. During streamed document work the profile timeout is an inactivity limit: received SSE heartbeats, reasoning deltas, output deltas, and final usage events prove that the connection is responsive and reset the timer. It is not a maximum duration for a complete generation request.

## Troubleshooting

When connection or verification fails, check these in order:

1. The model server or cloud account is actually available.
2. For a local server, the Server URL includes the correct scheme, host, port, and base path.
3. The provider account exposes at least one model compatible with the configured service.
4. A required API key is present in the secure or session-only store.
5. A firewall, proxy, VPN, or container boundary is not making `127.0.0.1` refer to the wrong machine.
6. For LAN llama.cpp, the server is reachable from the device and the unencrypted-LAN acknowledgement is set only when intentionally using HTTP.
7. The selected model supports the structured output and context size required by the task.

Use **Test connection** before starting a long document job. A failed profile test should be fixed at the provider boundary rather than retried repeatedly through document generation.

For implementation details, see [AI and document processing](../architecture/ai-and-document-processing.md).

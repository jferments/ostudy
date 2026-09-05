# AI providers and model profiles

AI is optional. Cards, editing, practice, scheduling, analytics, Search, imports, and backups do not need a model connection. Profiles become relevant when you request document analysis, flashcard generation, or study-guide generation.

ostudy connects to an existing model service. It does not install a local model, GPU runtime, or provider account, and it never silently switches a local task to a cloud provider.

## Implemented providers

| Provider family | Current application support |
| --- | --- |
| llama.cpp / Unsloth GGUF | Existing compatible local or deliberately configured LAN server |
| DeepSeek | Fixed cloud API and configured key |
| OpenRouter | Fixed API, account-visible model discovery, and adapter-specific routing/privacy policy |
| Dedicated Ollama, OpenAI, Gemini, Claude | Planned catalog entries, not implemented normal Add model choices |

A model available through OpenRouter is not evidence of a dedicated adapter for its original vendor. Provider/model availability outside the application can change; use the actual model discovery and connection test rather than assuming every model name is usable.

## Add and select a model

Open **Settings → AI/LLM** and choose **Add model**. Select an implemented provider. For llama.cpp enter its **Server URL**; for a key-requiring cloud provider enter the API key. Choose **Connect and load models**, select the returned model, and **Save profile**. Use **Test connection** before a long job.

The page is one vertical flow: default-model/task assignment, wrapping model list, selected-model controls, and advanced configuration. It is not a sticky model-sidebar workflow. The default selects a saved profile; it does not copy reasoning, credentials, timeout, or privacy settings from one profile to another.

The current flashcard-generation task default supplies the document learning workflows, including study guides. A separate study-guide-specific default selector is not required in the current app.

## Reasoning belongs to each model profile

Reasoning effort is stored with the selected profile, not as a global appearance/application setting. Choices reflect advertised metadata for that model. The application understands Automatic and a range of explicit effort levels, but it does not assume every provider/model supports every value.

Automatic lets the capability policy choose. Models that require reasoning cannot have it disabled. A saved choice that is no longer advertised must be reconciled rather than blindly sent as an unsupported request. Generation records requested and effective effort, so an adapted request is not confused with the user's original preference.

Changing the default model does not overwrite either model's profile settings. A previous global reasoning preference is handled by the native migration rather than retained as a second live setting.

## Credentials and advanced configuration

Advanced settings contain less common profile name, inactivity timeout, secure-device/session-only credential storage, optional llama.cpp server authentication, LAN cleartext acknowledgement, OpenRouter retention policy, testing/details, and deletion controls where applicable.

A newly typed key necessarily exists briefly in the entry form and crosses the write boundary to native storage. Saved keys are not read back into the form. Credential bytes are not stored in the SQLite library or ordinary backup/deck packages. Secure storage uses the platform's secret service; an explicitly selected session-only key remains in native memory and disappears when the process exits.

On a machine where secure storage is unavailable, a deliberate session-only credential can be used without pretending it will survive restart. Do not assume copying the library database copies keys. Connection-affecting changes invalidate old capability evidence; reconnect/test when the UI requests it. Removing/replacing keys and changing trust boundaries are deliberate operations, not an implicit side effect of choosing another default.

## Local and LAN servers

The usual local llama.cpp endpoint is:

```text
http://127.0.0.1:8080/
```

Start the server separately and use its actual address, port, and base path. The server owns loading the model and its tokenizer/chat template; the WebView does not invent another model-specific template.

Loopback means the device running ostudy. On a phone, `127.0.0.1` is the phone, not your desktop. Use a reachable trusted LAN address for a desktop server accessed from Android. Limit the server's network exposure and use authentication where appropriate. Public cleartext HTTP is not supported; non-loopback private-LAN HTTP requires explicit acknowledgement because it is not encrypted transport.

A local server can still be unavailable, misconfigured, too small in context, or unreliable at structured output. Local ownership is a privacy choice, not a guarantee of model quality.

## Cloud profiles

DeepSeek and OpenRouter use their fixed configured provider endpoints rather than accepting an arbitrary cloud URL. API usage is billed by the provider, not by a separate ostudy subscription. Cost depends on workload, model, retries, and current provider terms; inspect those terms before transmitting sensitive material.

For OpenRouter document requests, the adapter requests parameter-compatible structured output, denies provider data collection, and disables provider-side context compression. **Require zero data retention** adds a stronger routing constraint where supported. This may reduce eligible endpoints or cause a request to fail. The app must report that rather than silently relaxing the profile.

These request policies are not an independent audit of the external provider. Choose an appropriate service for the material you intend to process.

## Verification is more than connectivity

The connection test checks endpoint/authentication, model discovery/selection, and relevant text/structured-output capabilities. Results distinguish supported, unsupported, and unknown evidence. A successful HTTP response alone does not prove that a model can satisfy the document schema.

Evidence is tied to profile revision, credential epoch, and model identity. Changing those invalidates older evidence. The ordinary result appears first; **Technical details** exposes probe phases and capability information for diagnosis. Fix a failing provider test before repeatedly starting document generation.

## Data flow and privacy

Document parsing, rendering, and OCR occur locally. Generation sends bounded source text and selected visual evidence only when supported by the chosen model. It does not upload the original source as an opaque file. Generated cards/guides and canonical source derivatives are intentionally stored in the local library.

Provider requests run through the Rust gateway, not frontend HTTP. Redirects and public cleartext endpoints are rejected, response sizes are bounded, cookies are not a session mechanism, and routine diagnostics exclude keys, authorization headers, raw prompts/responses, reasoning text, and source bytes.

For streamed document work, the configured timeout is an inactivity limit between observable transport events, not a total wall-clock cap on an entire large job. Responsive stream events can keep work alive; cancellation and size limits still apply.

## Backup and device transfer

A complete library backup includes nonsecret profile metadata and task defaults, but not credentials. A restored device must verify/re-enter keys as needed. Standard trusted-device synchronization excludes provider configuration. The sender may explicitly include supported profile/default metadata and available saved credentials for that offer; secure storage and authenticated encrypted transport remain required for the credential path.

Ordinary theme/font/application settings stay device-local during sync. No prior pairing grants permanent permission to transfer credentials. See [Device sync](device-sync.md).

## Troubleshoot at the connection boundary

Check the server/account, endpoint scheme/address/port/base path, advertised model, key availability, network reachability, LAN policy, and structured-output/context capability. A firewall, VPN, container, or emulator may change which machine an address reaches. Avoid exposing an unauthenticated server publicly as a shortcut.

For a report, include the safe error, provider family/model, app build/platform, and whether discovery or a particular verification phase failed. Do not share API keys, authorization headers, private source text, or raw provider bodies. See [Documents](documents-and-ai-generation.md) and [native AI infrastructure](../architecture/ai-infrastructure.md) for the next layer.

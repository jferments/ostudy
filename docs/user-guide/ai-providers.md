# AI providers and model profiles

AI is optional. Cards, editing, practice, scheduling, Progress, Search, imports, and backups do not need a model connection. A provider becomes relevant when you make flashcards or study guides from documents, chat with Tutor, or use semantic search in Tutor.

ostudy connects to an existing model service. It does not install a local model, GPU runtime, or provider account, and it never silently switches a local task to a cloud provider.

## Implemented providers

| Provider | Current application support |
| --- | --- |
| llama.cpp / Unsloth GGUF | Existing compatible local or deliberately configured LAN server |
| DeepSeek | DeepSeek's cloud API and your API key |
| OpenRouter | OpenRouter's API, your API key, the models your account can see, and its routing and privacy policy |
| Claude | Anthropic's API and your API key; lists the Claude models that support structured output |
| Ollama, OpenAI, Gemini | Planned; not offered when you add a provider |

A model available through OpenRouter is not a dedicated adapter for its original vendor. Provider and model availability outside the application can change; use the model list and **Test model** rather than assuming every model name is usable.

## Add a provider

Open **Settings → AI** and choose **Add a service** in **Services**. Pick the **Service**. For llama.cpp enter its **Server address**; for a cloud service enter the **API key**, and leave **Keep the key on this device** on to save it in the system's secure storage, or turn it off to keep it for this session only. Choose **Connect**. Saved services are listed with **Ready**, **Needs a key** or **Off**; select one to change it, or use its menu to **Edit** or **Remove** it. **Refresh models** reloads the service's model list.

**More options** in a service hold **Use this service**, **Give up after** (the inactivity timeout) and **Remove API key**. A llama.cpp service also offers **Server needs a key** and, for a private-network address over plain HTTP, **Allow an unencrypted network connection**. An OpenRouter service offers **Only zero-data-retention routes**.

The first OpenRouter service you save with a key also fills any task that has no model yet with suggested OpenRouter models. That does not test them; change or test them like any other choice.

## Choose a model for each task

**Default for each task**, below **Services** on the same page, has one row per task:

| Task | Used for | Reasoning |
| --- | --- | --- |
| **Cards and guides** | Reading documents and image sets, making cards, study guides and a subject's study materials | Always off |
| **Tutor** | Tutor chat, including **Ask Tutor** from a study guide | Your choice; starts at Low |
| **Search** | Searching saved knowledge and conversations in Tutor | Not used; llama.cpp and OpenRouter only |
| **Voice** | Reading lesson steps aloud with **Listen** | Not used; OpenRouter only. The model is **Drama 3** (the default) or **Kokoro**, which adds a voice choice in the Reasoning column |

In each row choose the **Service** and then the **Model**. The choice is saved as soon as you make it. The row's menu has **Test model**, which checks that the selected model can do that task (a check mark and a message show the result, and a failure names the step that failed), and **Clear**, which leaves the task without a model. The **Search** row's menu also has **Embedding options** for dimensions and input policy. The **Voice** row's menu has only **Clear**: a voice is checked by listening to a lesson step, and each step it reads costs about a cent or less the first time, after which its audio is kept.

## Reasoning

Only the Tutor reasons. Its row has a **Reasoning** menu with **Automatic**, **Off** and the effort levels the model advertises; a model that cannot turn reasoning off does not offer **Off**. A saved choice the model no longer supports is shown as unsupported until you pick another.

Document work, cards and study guides always ask for no reasoning, because hidden reasoning shares the model's output budget and long chapters came back empty. A model that cannot turn reasoning off, such as some current Claude models, uses its lowest effort for that work. A document's **Details** shows the requested and effective reasoning for its run.

## Credentials

A newly typed key exists briefly in the entry form and is then handed to native storage. Saved keys are not read back into the form. Keys are not stored in the SQLite library, backups or deck packages. **Save securely** uses the operating system's secret service; **This session only** keeps the key in memory until oStudy closes. Where secure storage is unavailable, only **This session only** is offered.

Copying the library does not copy keys. Changing a provider's address, key or model makes earlier test results out of date; test the model again when asked. Removing a key or choosing another default never happens as a side effect of another change.

## Local and LAN servers

The usual local llama.cpp endpoint is:

```text
http://127.0.0.1:8080/
```

Start the server separately and use its actual address, port, and base path. The server loads the model and its chat template; ostudy does not invent another one.

Loopback means the device running ostudy. On a phone, `127.0.0.1` is the phone, not your desktop. Use a reachable trusted LAN address for a desktop server accessed from Android. Limit the server's network exposure and use authentication where appropriate. Public cleartext HTTP is not supported; private-LAN HTTP needs **Allow unencrypted LAN connection** because it is not encrypted.

A local server can still be unavailable, misconfigured, too small in context, or unreliable at structured output. Running locally is a privacy choice, not a guarantee of model quality.

## Cloud providers

DeepSeek, OpenRouter, and Claude use their fixed provider addresses; you cannot point them at another URL. The provider bills API usage, not ostudy. Cost depends on workload, model, retries, and current provider terms; read those terms before sending sensitive material.

For OpenRouter requests, ostudy asks for providers that support every parameter it sends, denies provider data collection, and turns off OpenRouter's context compression for document work. **Require OpenRouter zero-data-retention routing** restricts routing further; that can leave fewer eligible endpoints or make a request fail, and ostudy reports the failure rather than relaxing the setting. An OpenRouter model must pass **Test model** before document work uses it. Models that list image input also receive page images with the text once the test has confirmed they can read an image; a model tested before oStudy could send images needs **Test model** again to turn this on.

Claude lists only models that support structured output. Claude does not report cost, so usage shows tokens only, including prompt-cache reads.

These request policies are not an independent audit of the external provider. Choose a service appropriate for the material you intend to process.

## Testing is more than connecting

**Connect** checks the address, the key and the model list. **Test model** on the generation row also checks that the model can return ostudy's document format, with reasoning off as real document work uses it; a successful connection alone does not prove that. Fix a failing test before starting document work again and again. If a real document reaches the provider's output limit, ostudy treats that as a request that needs a smaller answer, not as a broken model.

## Data flow and privacy

Document parsing, rendering, and OCR happen on your device. Generation sends bounded portions of the extracted text and, when the tested model accepts images, selected page images. It does not upload the original file. Generated cards and guides, and the extracted text they come from, are stored in your local library.

Provider requests run in the native app, not the web view. Redirects and public cleartext addresses are refused, response sizes are bounded, and routine diagnostics leave out keys, authorization headers, prompts, responses, reasoning text, and source text.

The provider inactivity timeout limits the wait between signs of progress from the provider, not the total length of a large job. Cancellation and size limits still apply.

## Backup and device transfer

A whole-library backup includes provider settings and task defaults, but not keys; after a restore, enter or check keys as needed. Device sync leaves provider settings out unless the sender selects **API provider settings**, and sends keys only when the sender also selects **Saved API keys** and both devices have secure storage. Ordinary theme, font and application settings stay on each device. See [Device sync](device-sync.md).

## Troubleshoot at the connection

Check the server or account, the address, port and base path, the selected model, key availability, network reachability, the LAN setting, and whether the model supports structured output and enough context. A firewall, VPN, container, or emulator may change which machine an address reaches. Do not expose an unauthenticated server publicly as a shortcut.

For a report, include the error message, provider and model, app build and platform, and which test step failed. Do not share API keys, authorization headers, private source text, or raw provider responses. See [Documents](documents-and-ai-generation.md) and [native AI infrastructure](../architecture/ai-infrastructure.md) for the next layer.

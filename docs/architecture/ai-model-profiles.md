# AI model profiles and reasoning ownership

A model profile is the unit of connection, model selection, credential policy, reasoning effort, timeout, and provider-routing preference. A task default selects a profile ID; it does not become a second store for the profile's configuration. [AI infrastructure](ai-infrastructure.md) defines aggregate ownership and transport/probe boundaries.

## Per-profile reasoning

`ai_profiles.reasoning_effort` stores the selected effort. The native vocabulary includes Automatic and explicit effort levels, but a particular provider/model may advertise only a subset. The UI offers supported choices from model metadata rather than assuming that all values work everywhere.

Automatic resolves through capability policy. A model that requires reasoning cannot have it disabled. A stored choice no longer advertised by the selected model is reconciled against the valid advertised set; it must not be blindly emitted as an unsupported parameter. The policy does not invent an optional reasoning capability absent from evidence.

Generation provenance records **requested** and **effective** reasoning effort. This makes capability adaptation visible to diagnosis instead of pretending the exact requested value was always used. Reasoning output itself is not a generic frontend result or routine diagnostic payload.

Changing a task's default model leaves each profile's effort/key/timeout/privacy settings intact. Current document learning, including study guides, uses the flashcard-generation default; there is no requirement for a separate study-guide task-default UI in the current application.

## Migration from the legacy global setting

Migration `0008_ai_profile_reasoning.sql` adds profile-owned reasoning state, adopts a valid prior global preference for existing profiles according to its migration policy, uses the defined default where needed, and removes the old `app_settings.aiReasoningEffort` key. The released migration is not a file to rewrite for cosmetic cleanup.

The frontend's transitional optional input is removed before current Settings IPC rather than retained as a second authoritative global setting. Supported old input is compatibility at a boundary, not permission to show a global reasoning control again.

## Current Settings flow

Settings → AI/LLM is a vertical flow with task/default assignment, wrapping model list, selected-model controls, and advanced configuration. Add model asks for the implemented provider, relevant endpoint/key, connection/model discovery, model selection, and profile save/test. Less frequent trust/timeout/storage/routing choices remain contextual.

Only implemented adapters appear as normal choices: llama.cpp/Unsloth, DeepSeek, and OpenRouter. Other catalog enum entries remain planned. Nonsecret endpoint/model metadata is legitimate frontend state; saved credential bytes are not.

## Credentials, probes, and transfer

A newly typed key exists transiently in its input form and is written through the native secret boundary. The saved value is never read back into the form. Secure-store or explicitly selected session-only storage owns its subsequent lifetime. Ordinary SQLite library data, drafts, backups, and shared decks exclude key bytes.

Probe evidence is tied to profile revision, credential epoch, and selected model. Connection/trust/model edits invalidate old verification. A task must not treat an obsolete successful probe as current capability evidence.

Library backup includes allowed nonsecret profile metadata/defaults, not credentials. Normal device sync excludes provider configuration. A sender can explicitly opt into supported API metadata and available saved keys for one authenticated encrypted offer; the credential path is separate from ordinary package storage and requires secure storage at the receiving side. No pairing creates permanent consent.

## Sources and tests

Native ownership lives in `src-tauri/src/ai/profiles/`, `ai/reasoning.rs`, `ai/probes/`, and provider dialect modules. Frontend DTOs/controllers live in `src/ipc/aiProfiles.ts`, `src/controllers/aiProfilesController.ts`, and `src/screens/settings/AiLlmSettings.tsx`. Migration/contract tests protect legacy adoption and per-profile serialization; provider/probe tests protect capability resolution and staleness.

See [AI provider user guide](../user-guide/ai-providers.md), [native infrastructure](ai-infrastructure.md), and [device sync](device-sync.md). Profile structure must remain independent of the current Settings layout.

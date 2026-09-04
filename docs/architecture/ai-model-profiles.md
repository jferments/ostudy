# AI model profiles and per-model reasoning

## Ownership boundary

Each configured AI model is represented by one `ai_provider_profiles` row. The
profile owns its provider endpoint, model identity, credential reference,
privacy policy, request timeout, and requested reasoning effort. Application
appearance and scheduling settings do not own AI reasoning behavior.

The requested reasoning vocabulary is:

- `automatic`
- `none`
- `minimal`
- `low`
- `medium`
- `high`
- `xhigh`
- `max`

`automatic` asks the provider/model capability layer to choose its advertised
default. Explicit values remain requests rather than unchecked wire values:
document generation resolves them against the current model capability record.
A model that requires reasoning cannot be disabled; a stale or unsupported
request resolves to the lowest advertised valid effort. Optional models never
have an unsupported effort silently promoted.

Both the requested and effective effort are recorded in document-job
provenance. Switching the default model changes only the selected profile
identity. It never copies reasoning, timeout, retention, or credential state
between profiles.

## Migration

Public migration `0008_ai_profile_reasoning.sql` adds
`ai_provider_profiles.reasoning_effort`. For an existing installation, the last
valid legacy `app_settings.aiReasoningEffort` value is copied to every existing
profile. Invalid or missing legacy values become `low`. The global key is then
deleted.

The application-settings IPC no longer accepts or persists reasoning. The
TypeScript type retains an optional transitional property only so old fixtures
and in-memory objects can be saved safely; the wrapper strips it before IPC.

## Settings information architecture

The AI/LLM tab is one vertical flow:

1. default model/task assignment;
2. AI model list with Add model;
3. selected model connection and model/reasoning controls;
4. advanced provider, timeout, privacy, credential, test, and deletion controls.

The model list is a wrapping grid rather than a sticky sidebar or nested
horizontal scroller. The selected profile remains visible directly above its
configuration. Compact layouts collapse to one column.

Reasoning choices are generated from the selected model's advertised metadata.
Mandatory models omit `none`. When a provider catalog changes and an old value
is no longer supported, the UI requires selecting Automatic or another
advertised value before saving.

## Credential availability

Credential bytes never enter SQLite or frontend state. Secure storage remains
preferred. When Secret Service or KWallet is unavailable, the user may enter a
session-only credential. The in-memory credential takes precedence for the
running process, and cleanup of an inaccessible prior secure entry is deferred
without blocking the session.

## Backup and synchronization

Full local-library backups include AI profile metadata through SQLite while
credentials remain outside the database. Device sync deliberately excludes AI
profiles, task defaults, and credentials because endpoints and trust decisions
are device-local.

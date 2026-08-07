# ADR 0015: AI provider profiles, credentials, and Settings tabs

- **Status:** Accepted
- **Date:** 2026-08-07
- **Scope:** AI Stage 4.1

## Context

ostudy needs configurable access to existing local model servers and cloud APIs
without becoming responsible for installing models, Python, CUDA, Ollama,
llama.cpp, or another inference runtime. The existing Settings screen is one
large preference form backed by `AppSettings`; complete library backups contain
a coherent SQLite snapshot. Adding API credentials directly to that object or
the settings table would expose secrets to frontend reads and backups.

AI configuration also differs structurally from ordinary preferences:

- a learner may define several local and cloud profiles;
- profiles need stable IDs, optimistic revisions, and task assignments;
- provider kind and endpoint origin define a credential trust boundary;
- credentials have replace/remove/session lifecycles separate from metadata;
- future AI jobs must bind verification to an exact profile, model, and credential
  generation;
- saving appearance or scheduling preferences must not clear an unsaved profile
  draft.

## Decision

### Settings composition

Settings is a tabbed route with four accessible panels:

1. Storage/Backup
2. Appearance
3. AI/LLM
4. Review/Scheduling

A shared Tabs primitive owns tablist, tab, tabpanel, roving focus, arrow-key,
Home/End, compact scrolling, and accessible ID behavior. Appearance continues to
preview immediately. The Settings shell combines ordinary-preference dirty state
with AI-profile and credential-entry dirty state before allowing navigation away.

### Provider profiles

AI metadata is stored in dedicated `ai_provider_profiles` and
`ai_task_defaults` tables rather than `AppSettings`. Profiles have stable
32-character ostudy IDs, normalized active names, immutable provider kinds,
normalized endpoints, model IDs, auth modes, enabled state, bounded request
timeouts, credential epochs, optimistic revisions, and soft-deletion state.

Initial available profile kinds are llama.cpp and DeepSeek. Ollama, OpenAI,
Gemini, and Claude are visible as planned catalog entries but cannot be persisted
until their adapters exist. Unsloth is treated as a model origin/export workflow,
not as an API protocol; an Unsloth-exported GGUF is configured through a
llama.cpp profile.

Each AI task references at most one explicit profile. Stage 4.1 introduces the
`flashcard_generation` assignment. There is no fallback chain and no automatic
local-to-cloud substitution.

### Credential isolation

Credential bytes never enter SQLite, `AppSettings`, backups, diagnostics,
operation records, or read DTOs. A Rust `SecretManager` exposes only status,
resolve-for-native-request, set, and clear operations. Secure persistence uses
the operating system credential store through the Rust keyring boundary. When
secure persistence is unavailable, the user may explicitly choose a session-only
credential held in native process memory. There is no plaintext fallback.

The frontend receives statuses such as `missing`, `saved_securely`,
`session_only`, `not_required`, and `secure_store_unavailable`; it cannot read a
saved credential. Credential inputs clear immediately after submission.

SQLite and the operating-system keyring cannot share a transaction. Credential
mutations therefore increment a nonsecret credential epoch before the native
secret operation. A failed secret write leaves no plaintext data and merely
invalidates older capability evidence. Profile deletion removes the secret first,
then soft-deletes metadata and clears task assignments.

### Endpoint trust policy

Only complete `http` and `https` URLs are accepted. User information, query
strings, fragments, arbitrary schemes, and public cleartext HTTP are rejected.
Loopback HTTP is allowed. Private-network HTTP requires an explicit warning
acknowledgement. Built-in DeepSeek profiles use the fixed official endpoint.
Changing a credential-bearing profile to a different origin requires clearing the
credential first.

## Consequences

- complete library backups preserve provider metadata and task defaults while
  excluding credentials by construction;
- frontend code cannot retrieve or accidentally serialize persisted secrets;
- provider profiles can be referenced by later jobs without coupling them to
  ordinary visual or scheduling settings;
- profile edits and credential replacements invalidate stale verification state;
- Android and other platform ports can replace the secret backend behind the
  same interface;
- learners must explicitly choose a cloud profile rather than receiving a hidden
  fallback.

## Non-goals

Stage 4.1 does not perform network requests, list provider models, generate text,
process documents, install model infrastructure, create flashcards, or expose a
generic prompt interface.

## Validation

The decision is protected by migration tests, optimistic-revision and operation
replay tests, endpoint-policy tests, session-secret tests, backup-byte sentinel
tests, typed IPC tests, Settings dirty-state tests, credential-write-only UI tests,
Tabs accessibility tests, repository policy checks, and the ordinary self-hosted
qualification gates.

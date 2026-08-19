# ADR 0035: Simplified AI setup, semantic generation, explicit routes, and responsive guidance

- **Status:** Accepted for implementation
- **Date:** 2026-08-19
- **Scope:** Post-PR #54 AI and cross-platform UI overhaul

## Context

The post-PR #54 application has a secure and capable native AI/document pipeline, but several internal implementation boundaries are exposed directly in the UI. Creating an AI profile currently requires separate metadata, credential, model-discovery, model-save, and connection-test steps. Provider transport terminology, inactivity controls, enablement state, privacy details, and raw capability diagnostics compete with the primary setup task.

Document generation uses bounded semantic windows and native validation, but one generation request per window gives every window the same practical 12/16-card opportunity regardless of semantic density. Custom educational instructions are not yet represented as durable task policy, and the fixed prompt reserves cannot account for user-authored policy text.

The Flashcards workspace also contains redundant top-level card-creation navigation, indirect nested-route detection, viewport-only responsive breakpoints, a density preference that produces two parallel deck layouts, and mobile card previews that spend scarce width on labels and an arrow. Practice renders missing workload state as zeros. Contextual tutorials auto-start when unfinished anchors appear.

## Decision

### Explicit application routes

Navigation state is represented explicitly rather than inferred from rendered back buttons. A primary navigation action always targets the root of its section. Re-selecting Flashcards while a deck or editor is open returns to the deck library after the existing unsaved-change guards run. Flashcards workspace tabs similarly target workspace roots.

Legacy `create` workspace checkpoints normalize to the deck library. DOM mutation observers are not used to infer route depth.

### Deck-local card creation

The Create Cards workspace is removed. Single-card and bulk creation are actions on an open deck. `Add flashcard` is the primary action at the top-right of the card browser; `Bulk add` opens the existing bulk-entry workflow with the current deck fixed as destination.

### One compact deck presentation

The comfortable/compact preference is removed from settings and persistence. Deck rows use one compact visual layout while retaining accessible touch targets. The drag handle no longer has a vertical separator.

### Container-responsive workspaces

The outer shell owns phone/tablet/wide navigation mode. Workspace contents respond to their available inline size through container queries rather than assuming the viewport width equals usable content width. Intermediate tablet layouts are first-class, and simple-material/no-motion modes remain usable on e-ink devices.

Mobile card previews use exactly two compact lines: one `Q:` line and one `A:` line, each consuming the available horizontal width with CSS ellipsis. The arrow, visible row number, and separate Question/Answer label rows are removed.

### Practice workload state

Practice workload is an explicit idle/loading/loaded/empty/error state. Missing workload data is never converted to numerical zero. The initial effective scope restores a valid previous scope or selects all eligible active decks.

### Tutorials are opt-in actions

Interactive tutorials remain available but never auto-start because an anchor appears. Learners start or restart them explicitly from Help or a tutorial list. New installations default tutorials off; existing completion/dismissal progress remains valid.

### Simplified AI setup

The normal setup path is:

1. Select service/provider.
2. Enter API key or local connection information.
3. Select model.

Profile name, authentication transport mode, enabled state, inactivity timeout, LAN acknowledgement, routing policy, reasoning effort, context cap, credential persistence, and prompt preset are advanced settings. Cloud-provider explanatory prose belongs in Help rather than the task surface.

A native setup session temporarily owns connection metadata and credential bytes so model discovery and testing can occur before a durable profile exists. Commit creates the profile, stores the credential through the existing secret manager, assigns the selected model, and makes the first complete profile the default. Cancel leaves no incomplete profile or credential.

### Provider profile, task policy, and prompt preset are separate domains

A provider profile describes endpoint, credential boundary, model, and provider-specific routing/security policy. A task policy describes reasoning effort, context cap, image-input policy, coverage mode, and prompt-preset assignment for document flashcard generation. A prompt preset stores bounded user-authored outline and card-generation instructions.

Credential bytes remain excluded from SQLite, backups, read DTOs, and diagnostics. Prompt presets are non-secret learner configuration and are included in full-library backups.

### Safe prompt composition

Effective task prompts contain:

1. Immutable source-trust and task boundary.
2. Bounded user educational policy.
3. Immutable structured-output and validation contract.

Users can customize outline emphasis and card-generation emphasis. Curation policy, schemas, native validation, retries, and provider payload details remain application-owned. Diagnostics record only preset identity, revision, length, and hash.

Prompt overhead is estimated from the compiled prompt rather than fixed constants alone. A larger custom policy shrinks source-window capacity instead of creating provider context failures. Prompt policy identity participates in job and checkpoint fingerprints.

### Semantic learning-target generation

The outline/semantic platform produces normalized learning targets with kind, importance, evidence confidence, source spans, and prerequisite relationships. Generation is planned from uncovered high-value targets rather than a fixed card goal.

`max_cards_per_window` is replaced conceptually by `max_cards_per_response`: a transport safety ceiling. A sparse semantic region may justify zero cards; a dense region may receive multiple bounded target batches. Accepted cards record target/source coverage. One bounded gap pass may address uncovered high-priority targets. Generate More consults the durable coverage ledger rather than serializing the whole deck into prompt prose.

### OpenRouter routing controls

OpenRouter advanced settings may select automatic, preferred-with-fallback, or selected-only hosting providers and may require zero-data-retention endpoints. Parameter compatibility, data-collection denial, structured-output validation, and quality/precision safety constraints remain application invariants.

### Compact connection results

Connection testing renders one actionable summary row by default. Phase detail and safe diagnostics are available through an explicit disclosure.

## Consequences

- Core AI security and provider validation remain native-owned while setup becomes substantially simpler.
- Responsive behavior is deterministic across phone, tablet, desktop, and e-ink-oriented simple-material layouts.
- Navigation no longer depends on DOM inspection.
- Document generation cost remains bounded per request while semantic density, not an arbitrary count, determines the number of generation opportunities.
- Custom educational policy becomes portable and checkpoint-safe without exposing arbitrary prompt IPC.
- Several stored settings and legacy route values require versioned compatibility repair.

## Validation

Implementation is protected by route/root-navigation tests, mobile/tablet screenshot and interaction tests, settings migration tests, credential-isolation tests, AI setup-session tests, provider-routing fixtures, explicit workload-state tests, tutorial non-autostart tests, prompt-budget/checkpoint tests, learning-target coverage tests, bounded-gap-pass tests, backup/restore tests, and the existing self-hosted fast/full qualification gates.
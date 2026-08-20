# ADR 0035: Progressive AI configuration and explicit responsive workspace roots

- Status: Accepted for implementation
- Date: 2026-08-20

## Context

The post-PR #54 interface exposes implementation details that are useful for diagnostics but inappropriate in ordinary setup. AI profiles require separately saving provider metadata, credentials, and a model before the profile can be tested. Flashcards navigation also keeps nested deck/editor state inside child components, so reactivating an already-selected destination cannot reliably return to that destination's root. Responsive behavior is split across unrelated viewport breakpoints, which produces an inconsistent tablet layout. Practice workload tiles render unloaded data as zero, and tutorials launch contextually without an explicit learner request.

The requested product behavior is:

- ordinary AI setup is Provider → API key → Model;
- provider-specific policy and tuning controls are progressive disclosure;
- clicking a primary destination or workspace tab returns to its root after normal unsaved-work guards;
- card creation is contextual to a deck rather than a standalone workspace;
- deck/card lists use one compact visual density while retaining accessible touch targets;
- phone, tablet, and desktop layouts are coherent modes rather than accidental combinations of breakpoints;
- practice distinguishes unloaded, global, and selected-scope workload data;
- tutorials remain available but never launch automatically.

## Decision

### AI profile editor

The frontend owns one profile-editor state machine for create and edit modes. The primary surface contains only provider, API key, model, and a collapsed advanced-settings disclosure. A nonempty llama.cpp API key derives bearer authentication; an empty key derives no authentication. New profiles are enabled automatically.

Model discovery may run against an unsaved draft through a native command that receives validated endpoint/profile data and an ephemeral credential. Abandoning setup therefore does not leave an incomplete durable profile. Final create is presented as one save operation; the controller compensates by deleting a newly created profile if credential persistence fails.

Advanced settings are provider-aware and may include profile name, server address, credential persistence, inactivity timeout, insecure-LAN permission, OpenRouter zero-data-retention routing, context-window override, and reasoning effort. Generic sampling controls and arbitrary provider JSON are not exposed.

Context-window override is a maximum ostudy may use. When both a discovered context and an override exist, the effective context is the smaller value. Reasoning effort becomes profile-local and is resolved against current model capabilities without silently promoting optional reasoning to a more expensive unsupported level.

Connection tests show a compact status summary by default. Probe phases, capabilities, and the redacted diagnostic log remain available behind disclosures.

### Workspace routing

Flashcards navigation uses explicit nested route state rather than DOM inspection for a back button. Activating a workspace root is an intentional navigation action even when that workspace is already selected. Existing dirty-editor and active-practice guards continue to run before the route is reset.

The Create Cards workspace is removed. Add flashcard and Bulk add are deck-detail actions above the card list and reuse the existing card editor and bulk-entry modal.

### Responsive layout

The application shell exposes semantic compact, medium, and wide layout modes. Shell mode is based on viewport width; feature layouts use container-responsive behavior for their actual available width. Medium mode retains labeled primary navigation rather than an icon-only rail. Visual compactness does not reduce minimum touch targets.

Card-list previews use a two-line Q/A representation on compact widths and remove the decorative arrow. The deck move handle retains its icon/focus affordance without a permanent separator. The comfortable/compact persisted preference is removed and the compact layout becomes canonical.

### Practice and tutorials

Practice loads a global all-active-decks workload independently from the selected-scope session plan. Unloaded or failed workload state is never rendered as a real zero. The native workload read accepts an empty scope as all active decks, while session creation still requires an explicit deck or tag selection.

Tutorials launch only through an explicit action. The existing module registry, anchor placement, completion state, and restart behavior remain available when tutorials are enabled.

## Consequences

- AI profile persistence and document-planning fingerprints gain profile-local context/reasoning fields and require a data migration.
- The old global AI reasoning and deck-density settings are removed after compatibility migration.
- Flashcards checkpoints require normalization of the removed `create` workspace and explicit nested-route persistence.
- Tests must cover draft model discovery, compensated profile creation, active-tab root navigation, compact Q/A previews, global versus selected practice workload, manual-only tutorials, and compact/medium/wide layouts.
- Provider trust boundaries, native credential ownership, structured-output enforcement, scheduling, and analytics authority remain unchanged.

## Non-goals

- Adding new AI providers.
- Exposing arbitrary generation/sampling parameters.
- Redesigning scheduling semantics.
- Treating a specific tablet model or user agent as a special platform.
- Removing detailed provider diagnostics; they are collapsed, not discarded.

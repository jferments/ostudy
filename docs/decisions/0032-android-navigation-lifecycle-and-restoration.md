# ADR 0032: Android navigation, lifecycle, and restoration

- **Status:** Accepted
- **Date:** 2026-08-12
- **Scope:** Android phase A2

## Context

Desktop navigation is mostly process-local React state. Android may recreate an activity for configuration changes or terminate the process while the application is backgrounded. System Back must first close ostudy overlays and nested workflows, while Back at the root should remain owned by Android and participate in predictive back-to-home behavior.

## Decision

Android lifecycle observations are delivered through the internal mobile plugin and normalized into the shared `LifecycleCoordinator`. Lifecycle delivery is best effort; durable drafts, idempotent practice operations, managed-file ledgers, and startup reconciliation remain the correctness boundary.

ostudy stores a small device-local, versioned UI checkpoint in app-private storage. It contains only navigation destinations, entity IDs, active practice identity and position, revealed state, and pending idempotent practice request identifiers. It does not contain card text, rich documents, images, credentials, or arbitrary HTML. Editor content remains in the durable-draft subsystem. The checkpoint is excluded from complete-library backup and future content sync.

System Back is resolved in this order:

1. Android-owned system UI such as a picker or Sharesheet;
2. the top ostudy overlay;
3. the current nested editor or detail route;
4. active-practice termination policy;
5. the current tab stack;
6. prior tab history;
7. Android back-to-home at the Decks root.

JavaScript continuously reports whether an ostudy Back handler is available. Android intercepts only while one is available; otherwise it delegates directly to the platform so predictive Back remains correct.

Practice restoration reuses the durable `practice_sessions`, ordered `practice_session_cards`, view/reveal state, review events, start operation IDs, and review submission IDs. Pending start or review requests are checkpointed before invocation and safely replayed with the same identity after process death. Rust derives the canonical resume position from the database and rejects stale or conflicting requests.

## Consequences

- Activity recreation and abrupt process death no longer lose recoverable editor or practice state.
- Saved library data remains independent of device-local presentation state.
- Android Back feels native without giving Kotlin ownership of application navigation.
- The same checkpoint and route model can later support iOS scene restoration.

## Validation

Instrumentation tests cover overlay dismissal, nested-route Back, root back-to-home, rotation, background/resume, forced process death during every durable editor, revealed practice cards, pending review replay, completed sessions, software-keyboard insets, phone and tablet layouts, and rejection of stale checkpoint references.

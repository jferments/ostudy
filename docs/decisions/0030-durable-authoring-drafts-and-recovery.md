# ADR 0030: Durable authoring drafts and recovery

- **Status:** Accepted
- **Date:** 2026-08-11
- **Scope:** Cross-platform preparation phase P2

## Context

Managed image drafts already survive crashes through a SQLite/file ledger, but card, template, bulk-entry, and AI profile metadata live only in React state. Closing a desktop window or losing a mobile WebView can therefore discard substantial work. The current 24-hour asset-draft expiry and backup prohibition assume every draft is transient.

## Decision

ostudy stores typed, versioned editor drafts for:

- card creation and editing;
- template creation and editing;
- bulk card entry;
- AI provider profile metadata.

Credential text is never part of a durable draft. Ordinary appearance/review preference previews remain memory-only in this phase.

A draft records kind, optional target entity, base revision, context entity, payload schema version, payload hash, optimistic draft revision, state, and timestamps. Payloads may be incomplete but must satisfy bounded structural validation and the same rich-content safety allowlist used by canonical authoring.

Each editor draft owns one persistent managed asset draft. Draft saves atomically replace the referenced-asset set and retire unused staged assets. Persistent editor assets do not expire merely because 24 hours passed.

Autosave is serialized and revision-safe. An older completion may not overwrite newer local state. Lifecycle events request an immediate flush, but frequent autosave and startup reconciliation remain the real safety mechanism.

Final Save verifies the draft and target base revision, applies canonical validation, commits the canonical entity, promotes assets, and retires the draft through one use-case boundary. A changed base entity produces a recoverable conflict rather than silent overwrite. Explicit Discard retires the draft and unused assets.

Active persistent drafts and their staged assets are included in complete library backups. They are device-local and excluded from the first synchronization protocol.

## Consequences

A successfully autosaved draft can remain when the user navigates away; only unsaved local changes or an autosave failure require a destructive-navigation warning. Editors can recover after process loss without weakening canonical validation.

## Non-goals

This decision does not add collaborative editing, rich-text CRDTs, or automatic conflict merging.

## Validation

Tests cover create/recover/save/discard/commit, optimistic revision conflict, base-entity conflict, asset retention and cleanup, lifecycle flush, backup/restore, malformed payload rejection, and proof that credential text is never serialized.

# ADR 0027: Opaque external files and managed content protocol

- **Status:** Accepted
- **Date:** 2026-08-11
- **Scope:** Cross-platform preparation phases P0–P2

## Context

Desktop file dialogs currently return absolute paths to React. Those strings then pass through controllers and IPC into import/export commands. Android commonly returns `content://` resources and iOS document providers return security-scoped URLs whose permissions may expire. A durable application contract cannot assume every selected item is a reopenable path.

Managed card images have a good durable identity (`study-asset://<asset-id>`), but hydration currently exposes absolute managed paths to React before converting them to WebView URLs.

## Decision

External file references are temporary platform values and never durable feature values.

### Imports

A platform adapter selects a source and streams it into ostudy-owned staging. The staging service enforces purpose-specific size limits, hashes the bytes, detects the actual content kind, flushes the file, and records a ledger row. Feature and domain APIs receive only a `stagedFileId` plus safe metadata.

The existing `.studydeck`, `.ostudy.json`, `.ostudybackup`, PDF, and image validators remain authoritative. Successful imports consume the staged item transactionally where possible. Validation failure keeps it available for bounded retry until release or expiry.

### Exports

Domain services first build and validate an artifact in ostudy-managed storage. They return a `preparedExportId` and summary. A platform publisher then asks the user for a destination or share target and streams the prepared bytes there. Canceling publication does not require rebuilding the artifact.

### Managed images

React receives asset identity and metadata, never an absolute path. A narrow custom protocol resolves an asset ID through the managed asset ledger and serves only verified ready or active-draft bytes with the recorded MIME type.

### Security and recovery

- File type is determined from content/package metadata, not extension alone.
- Managed relative paths are derived from transfer IDs and checked against ledger rows.
- Partial, unknown, expired, and interrupted transfer files are reconciled at startup.
- User cancellation is a normal null result, not a domain failure.
- External paths and URIs are excluded from drafts, backups, diagnostics, and persisted content.

## Consequences

The same import/export use cases work with desktop paths, Android content URIs, iOS document providers, cloud-backed sources, share sheets, and future nearby transfer without changing card, deck, document, or backup validation.

## Non-goals

This decision does not define QR sharing or personal sync transport. Those later features may reuse prepared exports and staged imports.

## Validation

Tests cover cancellation, delayed or disappearing sources, streaming limits, hash/type mismatch, interrupted staging, retry identity, startup cleanup, prepared-publication failure, and rejection of arbitrary asset protocol paths.

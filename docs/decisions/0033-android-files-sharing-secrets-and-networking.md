# ADR 0033: Android files, sharing, secrets, and networking

- **Status:** Accepted
- **Date:** 2026-08-12
- **Scope:** Android phase A3

## Context

Android document providers expose `content://` resources rather than durable filesystem paths. External sources may be cloud-backed, slow, disappear after the picker closes, or report no length. Android exports and Sharesheet transfers also require temporary URI grants rather than public paths. Provider credentials need protected device storage, and local AI endpoints require clear network diagnostics without duplicating the shared Rust gateway.

## Decision

The Android external-file adapter uses the Storage Access Framework. Rust creates a bounded app-private staging destination and asks Kotlin to copy the selected `ContentResolver` stream there on `Dispatchers.IO`. Kotlin validates that the destination remains under the ostudy sandbox and returns only safe metadata. Rust remains responsible for byte limits, SHA-256, content-kind detection, package validation, transfer-ledger transitions, and import consumption.

Exports remain prepared and validated in Rust-managed storage. `CreateDocument` streams a prepared artifact to the selected provider. Android sharing is a separate operation: a narrowly scoped `FileProvider` exposes only prepared-export files through temporary read-only URI grants and launches `ACTION_SEND`. A launched Sharesheet is not falsely described as confirmed delivery.

Incoming `ACTION_SEND` content is copied immediately into an app-private incoming cache while the temporary grant is valid. The app presents a confirmation before adopting it into the ordinary staged-import ledger. Native URIs are never persisted in learner data, drafts, backups, diagnostics, or domain IPC.

The Android secure-secret adapter uses a non-exportable AES-256-GCM key in `AndroidKeyStore`. Encrypted records are stored in an app-private preferences file excluded from platform backup. AI credentials remain write-only in the UI, session-only values remain in Rust memory, and Keystore invalidation maps to a stable recoverable product state.

Cloud and LAN provider requests continue through the shared Rust `reqwest` gateway and endpoint policy. Kotlin reports connectivity, metering, and platform permission state only. Release WebView and Java network configuration does not globally permit cleartext traffic. Explicit LAN HTTP acknowledgement remains a shared Rust rule. The adapter anticipates Android's dedicated local-network permission without targeting that later SDK requirement prematurely.

No broad storage permission, all-files access, or native database access is permitted.

## Consequences

- Imports work with Downloads, removable storage, and cloud document providers without leaking URIs.
- Prepared exports can be saved, retried, shared, and later reused by QR transfer.
- Credentials gain Android hardware-backed protection where the device supports it.
- Network policy remains consistent across Linux and Android.
- Android native code remains an I/O adapter rather than a second backend.

## Validation

Tests use deterministic document-provider, share-sender, and share-receiver fixtures. They cover cancellation, slow and unknown-length streams, oversize input, interrupted copies, provider write failure, URI-grant expiry, incoming-share confirmation, FileProvider scope, Keystore persistence and invalidation, uninstall/reset behavior, HTTPS cloud probes, acknowledged LAN HTTP, metered/offline transitions, and absence of broad storage or backup permissions.

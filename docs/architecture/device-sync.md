# Device synchronization architecture

ostudy device synchronization is an explicit, local-network transfer between two installations. It is **not** a cloud account, background replication service, or live CRDT. A user chooses when one device will send a synchronization offer, and another device chooses whether to accept it.

The implementation shares the existing SQLite/domain model, but never copies a live SQLite database between running applications. The sender creates a coherent point-in-time synchronization package, authenticates and encrypts the transport, and the receiver merges the package through the same integrity-aware application data model used elsewhere.

## Product semantics

Protocol version 2 uses a **one-way offer** model:

1. The sending device creates a short-lived invitation.
2. The invitation records the sender identity, private-LAN endpoint, one-time pairing secret/key-agreement material, expiry, and the sender-selected transfer scope.
3. The receiving device scans the QR code or pastes the invitation text.
4. Both devices authenticate the invitation/session and derive fresh transport keys.
5. The sender transmits its selected snapshot.
6. The receiver merges the snapshot locally and returns only a completion acknowledgement.

The receiving device does **not** upload its library as part of accepting the offer. A reverse transfer is a separate explicit action: the other device creates its own new offer when the user actually wants to send changes back.

On Android, scanning a valid QR code accepts the offer and begins receiving immediately. Pasted invitation text uses an explicit **Accept sync offer** action because paste is not itself an intentional camera acceptance gesture.

## What synchronizes

The ordinary study-library scope includes the durable entities required for the same study content and learning history to exist on another installation, including:

- deck folders and decks;
- notes/cards and card presentations;
- tags and note/tag relationships;
- card templates, template fields/rules, and template assets;
- managed image metadata and the corresponding validated image files;
- card schedules;
- practice-session snapshots needed by durable practice/history semantics;
- review events and historical attribution rows.

The merge table allowlist lives in `src-tauri/src/device_sync/merge.rs`. A database table is not synchronized merely because it exists in the application schema.

### Device-local settings are never synchronized

`app_settings` is intentionally **not** part of the synchronization allowlist. The sender also removes every `app_settings` row from the packaged SQLite snapshot before the package can be transmitted.

This keeps platform- and device-specific preferences independent, including themes, font choice, UI scale, motion preferences, density/layout choices, and other appearance/application settings. An empty remote settings table is never interpreted as a request to delete local settings.

PR #51 briefly exposed `app_settings` conflicts during pre-merge physical testing. Protocol v2 retires those old conflict records while preserving the local device's value.

### API provider settings are a separate sender-controlled opt-in

The only settings category currently eligible for synchronization is AI/API provider configuration, and it is **off by default** for every offer.

When the sender leaves **Include API provider settings and saved credentials** off:

- `ai_provider_profiles` is removed from the outgoing snapshot;
- `ai_task_defaults` is removed from the outgoing snapshot;
- provider probe/capability/operation cache rows are removed;
- no secure credential bytes are collected.

When the sender explicitly enables the option:

- provider profile metadata and task defaults may be merged;
- ephemeral probe/capability/operation results are still omitted;
- SQLite credential-storage markers/epochs are stripped from the package;
- saved credential bytes, when available, travel separately through the authenticated encrypted session and are written through the receiver's secure secret store.

The receiver does not need a matching sharing toggle. Its secure secret store must be technically available before credential bytes can be accepted. File-based snapshots/backups never gain credential bytes from this feature.

## Snapshot/package boundary

`device_sync/package.rs` creates a package from a SQLite backup snapshot rather than copying the live database file. Before serialization it sanitizes the snapshot according to the offer scope.

The package contains:

- versioned package metadata;
- one coherent SQLite snapshot as a bounded blob;
- validated managed-image payloads with IDs, relative paths, MIME types, sizes, and SHA-256 hashes.

On receipt, ostudy validates package format/version, SQLite integrity, schema compatibility, asset paths, MIME signatures, byte sizes, and hashes before merge/install work proceeds.

The package format version is independent from the application database schema version. Protocol-v2 clients reject unsupported/expired protocol-v1 invitations and require a fresh offer from an updated sender.

## Pairing and transport security

Pairing uses stable device identity plus fresh per-offer session material:

- Ed25519 device identity signatures;
- X25519 ephemeral key agreement;
- HKDF key derivation;
- ChaCha20-Poly1305 encrypted frames;
- a fresh one-time random pairing secret embedded in the short-lived invitation;
- authenticated transcript binding for session identity, endpoint, expiry, and sender-selected API scope.

The transport is framed **TCP**, not HTTP. The listener binds locally and advertised endpoints are restricted to private, link-local, or loopback addresses. Invitations are short lived and bounded in size.

There is no long-term reusable transport secret encoded in future QR codes. Stable peer identity and merge baselines recognize devices across sessions; every offer receives new ephemeral/session cryptographic material.

## Android QR acquisition

Android QR acquisition is implemented by the private ostudy Tauri platform plugin using the embedded ZXing scanner. It is QR-only and does not depend on an external barcode application.

This is separate from Android document OCR: PDF/text OCR uses bundled ML Kit Text Recognition v2, while pairing QR capture uses ZXing.

The callback-based Android Activity result is awaited from a Tauri blocking worker rather than from a synchronous command/event-loop wait. This prevents the native activity-result/lifecycle delivery from being stalled by the command that opened the camera.

## Merge semantics

The receiver uses deterministic three-way row merging when it has a saved common baseline for the peer:

- unchanged-on-one-side / changed-on-the-other takes the changed value;
- independent field edits can merge automatically;
- same-field concurrent edits create an explicit conflict;
- delete-versus-edit preserves the surviving row and records a conflict rather than silently discarding content;
- managed-asset identity/content mismatches fail rather than selecting arbitrary image bytes.

Conflict records store both candidate payloads and the deterministically selected working value. The user can later choose the appropriate side or delete the record. Resolved decisions become ordinary local data and can be sent in a future offer.

After the receiver merges an offer, it records the resulting peer baseline. After the sender receives the receiver's completion acknowledgement, it records the exact snapshot it sent as its baseline for that peer. This preserves useful three-way context for a later reverse offer **without uploading the receiver's library during acceptance**.

## Failure and lifecycle boundaries

A synchronization session has explicit waiting, connecting, transferring, merging, completed, failed, expired, or canceled state. Temporary transfer workspaces live under app-private storage and are removed when the operation completes or unwinds.

Important boundaries:

- accepting an invitation is not a background subscription;
- a process kill ends the active network session;
- the invitation expires rather than becoming a permanent listener;
- database/package validation failures do not silently downgrade into partial success;
- credential transfer is never inferred from a previously paired device—it must be selected again by the sender for the new offer.

## Trust removal

Removing a trusted device removes its peer record and stored common-baseline/conflict/session relationship data according to the sync-store rules. It does not delete the learner's decks/cards merely because those records previously arrived from that peer.

A later transfer to or from that installation requires a fresh invitation and authentication.

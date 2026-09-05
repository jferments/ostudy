# Trusted-device synchronization architecture

Sync is an explicit local-network exchange between trusted installations. It is not a hosted account, automatic cloud backup, continuous background replication, shared SQLite file, or generic merge of every table in the application. [The user guide](../user-guide/device-sync.md) describes the actual offer/receive workflow; [the comprehensive architecture](overview.md) describes the full data graph.

## Offer, receive, acknowledge

The sender creates one short-lived outgoing offer and displays a QR plus text invitation. The receiver scans or explicitly submits the text. A successful scan accepts that sender-scoped offer, begins the authenticated connection, and imports/merges its allowed data after any necessary conflict decisions. Acceptance never secretly uploads the receiver's whole library in reverse.

A reverse exchange is a separate new offer from the other installation. The sender records the exact acknowledged snapshot as its baseline; the receiver records its post-merge baseline. This preserves meaningful subsequent common-base comparisons without pretending the initial one-way transfer was bidirectional.

Only one unclaimed outgoing offer is retained; a new one replaces that unclaimed invitation. Overlapping sessions for the same peer are rejected while active. A failed connect can retry a still-valid invitation rather than leaving the UI permanently stuck in an attempt state.

## Identity and authenticated transport

The native protocol uses stable Ed25519 identity, fresh X25519 exchange material, HKDF-derived session keys, ChaCha20-Poly1305 authenticated frames, and a one-time pairing secret. The signed/bound invitation/session context includes peer/endpoint/expiry and relevant scope. A QR is a short-lived capability to accept an offer, not a permanent password.

Transport is framed encrypted TCP over an explicitly reachable private network. The listener uses the bounded port range **49251–49258**. The receiver connects to the sender; the party displaying the QR needs the reachable inbound listener for that offer. A successful transfer in one direction does not prove the opposite direction's firewall/routing is configured.

Do not expose the listener directly to the public Internet, disable authentication, or treat being on the same Wi-Fi as sufficient trust. A VPN, AP client isolation, host firewall, container/VM routing, or multi-interface address choice can still prevent private reachability. Encryption does not make the receiving device trustworthy or erase content after it is transferred.

## Android QR acquisition

Android uses **Google Code Scanner through Play services**, not the retired ZXing-based acquisition description. The scanner owns its UI/camera permission; ostudy receives a bounded decoded result and validates its protocol natively. Pasted invitation text is the supported fallback when scanner/Play-services availability prevents scanning.

This is separate from ML Kit document OCR. Scanner success is not invitation acceptance until the decoded value passes expiry, identity, scope, and authentication checks. The frontend does not possess private signing keys merely to render or consume the QR.

## Allowed data graph

The sync store/merge code explicitly owns its allowlist. Supported study data includes decks/folders, notes/cards, tags, templates, managed images, schedules, practice sessions and their card records, review history, and associated historical attribution.

**The current document/study-guide learning-artifact graph is not a full supported sync promise.** Source document files, canonical derivatives, generated guide revisions, learner overlays, and topic relationships require a coordinated versioned sync extension. Their existence in a newer local database or whole-library backup does not put them automatically on the merge allowlist.

A coherent outgoing snapshot is sanitized rather than sending a raw live database. Ordinary `app_settings` are excluded/stripped, keeping appearance and application preferences device-local. Probe/operation caches and unrelated transient state are not learner-data merge authority. Version/schema validation and graph checks prevent unsupported package contents from being silently interpreted as a complete library replacement.

## Optional API settings and credential transfer

Normal offers exclude provider configuration. The sender can explicitly opt into supported provider-profile metadata, task defaults, and available saved credentials **for that offer**. Consent defaults off each time; a previous pairing does not imply permanent authorization.

Credential bytes travel through a separate authenticated secure-store boundary. They are not written into the SQLite snapshot, ordinary file fallback, shared deck, or complete backup. Eligible profile references and required key availability are validated. The receiving device must have usable secure storage for the saved-key path; a missing secret service is not permission to store keys in plaintext settings.

A newly entered key can exist transiently in a form, but saved keys are not read back into the UI. Standard nonsecret profile metadata and endpoint/model projections are different from credential bytes. The [AI infrastructure reference](ai-infrastructure.md) explains their ownership.

## Merge semantics

The receiver compares local and incoming rows with a stored common base. One-sided changes can apply, independent field changes can merge, and concurrent changes to the same field or delete-versus-edit cases become explicit conflicts. This is a versioned application merge policy, not a generic last-writer-wins database copy.

Managed-image identity collisions or byte/integrity disagreement fail rather than silently selecting an arbitrary file. The final merge validates the complete allowed graph and commits it transactionally. Related entities must remain consistent with foreign keys, schedule/history identities, and current lifecycle policy.

Removing trust deletes the relationship/baseline state according to the native policy, not cards or history already imported into the local learner's library. Sync is not remote revocation of another person's data.

## Cancellation and restart

Waiting, connecting, and transferring sessions can be canceled through the task/session signal and socket shutdown. Per-peer exclusivity prevents a second action racing the same baseline while one is active.

The short final database merge is intentionally non-cancellable. Interrupting it halfway would be a worse data contract than finishing one atomic commit. UI wording must distinguish a cancelable transfer from that final applying boundary.

Startup reconciles stale process-local sessions; old sockets and invitation acceptance callbacks do not survive process death. A retry uses actual current native state. The frontend must not make an obsolete completion overwrite a newer session's display.

## File fallback and backup distinctions

The supported file fallback exchanges the versioned study-data package and its validated managed assets, not a live SQL database mounted by both devices. Saved credentials remain excluded from that ordinary file path. Validate package version and scope rather than inferring that a filename is a safe full backup.

A `.studydeck` imports content with new local schedules. `.ostudybackup` replaces a coherent personal library through staged restart recovery. Sync merges its narrower allowed study graph using identities/common bases. These operations cannot be substituted without changing history/privacy semantics.

## Ownership, diagnostics, and tests

Native owners are `device_sync/mod.rs`, `store.rs`, `merge.rs`, `package.rs`, `transport.rs`, `crypto.rs`, `credentials.rs`, `network.rs`, and `cancellation.rs`. The frontend Settings component presents state and intent; the platform QR plugin acquires input. Neither owns crypto or relational merge rules.

Tests cover protocol/identity framing, package/graph validation, field conflicts, baseline handling, managed-image integrity, credential opt-in, cancellation and overlap. Static checks enforce QR acquisition, stable port and UI scope contracts. Android instrumentation checks the native plugin route; real peer/OEM/firewall/Play-services behavior remains explicit manual acceptance when not exercised.

Diagnostics report safe phase, endpoint/session outcome, and bounded errors without keys, invitation secrets, raw package contents, or personal source text. A storage/details screen identifies local diagnostic paths. Review exported diagnostics before sharing; metadata may still identify a device, model, or filename.

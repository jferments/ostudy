# Trusted-device synchronization architecture

Synchronization is an explicit exchange between trusted ostudy installations. It is not a hosted account, automatic cloud backup, continuous background replication, shared SQLite file, or generic merge of every table in the application. [The user guide](../user-guide/device-sync.md) describes the actual offer/receive workflow; [the comprehensive architecture](overview.md) describes the wider application data graph.

The synchronization architecture deliberately separates **sync semantics** from **transport mechanics**. Local-network TCP and USB are ways to establish a byte path between two installations; neither transport owns pairing, identity, encryption, packages, conflict policy, credentials, or merge behavior.

## Layering and ownership

The synchronization subsystem has four conceptual layers:

1. **Sync orchestration and protocol** own one-way offers, session state, pairing transcripts, peer identity, acknowledgements, and progress/error transitions.
2. **Secure framing** owns handshake message framing, authenticated encrypted frames, bounded control messages, package streaming, and file hashes.
3. **Transport adapters** own how bytes are read/written and how a blocked transfer is interrupted. `SyncIo` is the byte-stream boundary; `SyncAbort`/`SyncAbortSource` are the cancellation boundary.
4. **Package and merge services** own the explicit data allowlist, common-baseline comparison, conflicts, managed assets, credential scope, integrity validation, and atomic commit.

The core invariant is:

> A transport may establish, configure, read, write, close, or abort a connection. It must not acquire synchronization semantics.

Consequently `SecureChannel` must not own `TcpStream`, cancellation must not know about `Shutdown::Both`, and a future iOS/native-USB adapter must not need copies of package, merge, crypto, or conflict code. TCP-specific socket setup and shutdown remain in the TCP adapter. A native USB implementation can satisfy the same stream/abort contracts without changing the higher layers.

## Offer, receive, acknowledge

The sender creates one short-lived outgoing offer and displays a QR plus text invitation. The receiver scans or explicitly submits the text. A successful scan accepts that sender-scoped offer, begins the authenticated connection, and imports/merges its allowed data after any necessary conflict decisions. Acceptance never secretly uploads the receiver's whole library in reverse.

A reverse exchange is a separate new offer from the other installation. The sender records the exact acknowledged snapshot as its baseline; the receiver records its post-merge baseline. This preserves meaningful subsequent common-base comparisons without pretending the initial one-way transfer was bidirectional.

Only one unclaimed outgoing offer is retained; a new one replaces that unclaimed invitation. Overlapping sessions for the same peer are rejected while active. A failed connect can retry a still-valid invitation rather than leaving the UI permanently stuck in an attempt state.

The protocol-v2 invitation remains transport-neutral. It binds the sender's reachable private endpoint into the signed/authenticated pairing context. A LAN endpoint and a USB-network endpoint therefore use the same invitation grammar and cryptographic transcript. Stable device identity and stored merge baselines belong to the installation, not to the IP address, Wi-Fi network, USB cable, or transport used for a particular transfer.

## Identity and authenticated channel

The native protocol uses stable Ed25519 identity, fresh X25519 exchange material, HKDF-derived session keys, ChaCha20-Poly1305 authenticated frames, and a one-time pairing secret. The signed/bound invitation/session context includes peer/endpoint/expiry and relevant scope. A QR is a short-lived capability to accept an offer, not a permanent password.

The current network-backed adapters carry the framed channel over private TCP. The listener uses the bounded port range **49251–49258**. The receiver connects to the sender; the party displaying the QR needs the reachable inbound listener for that offer. A successful transfer in one direction does not prove the opposite direction's firewall/routing is configured.

Do not expose the listener directly to the public Internet, disable authentication, or treat physical USB presence or membership on the same Wi-Fi as sufficient trust. USB is an additional path to an authenticated peer, not a replacement for identity verification and encryption.

## USB synchronization

### Why USB is a transport, not a second sync engine

USB synchronization must reuse the same protocol and merge machinery as LAN synchronization. We explicitly reject architectures such as `wifi_sync.rs` and `usb_sync.rs` that independently implement card reconciliation, conflict resolution, credential handling, or package semantics.

The preferred shape is:

```text
                       synchronization protocol
                identity / crypto / package / merge
                                  |
                              SecureChannel
                                  |
                         SyncIo + SyncAbort
                           /              \
                    TCP byte path      native USB path
                    /          \
             private LAN    USB networking
```

The first USB backend uses **point-to-point USB networking** where the operating system exposes it. The cable creates a private network interface and ostudy uses the existing TCP adapter on that interface. This gives USB the same framing, timeouts, cancellation, encryption, authentication, package transfer, and acknowledgement behavior without inventing a USB-specific wire protocol.

A native raw-USB backend remains possible for platform combinations where USB networking is unavailable or inappropriate. Such a backend implements the stream/abort boundary and remains invisible to the synchronization protocol above it.

### Current endpoint selection

Automatic LAN address discovery continues to serve normal Wi-Fi/Ethernet offers. For USB-network offers, the sender currently selects **USB cable** in Settings and supplies the private address assigned to the sender's USB-network interface. Requiring an explicit address is intentional in the first implementation: guessing from interface names such as `rndis0`, `usb0`, `enx...`, Windows adapter descriptions, or Apple interface naming would create a brittle cross-platform heuristic inside synchronization code.

Automatic USB-interface discovery belongs behind a platform adapter. When added, Linux/Windows/macOS native code, Android Kotlin, and future iOS Swift may discover or prepare their respective USB link differently, while the sync UI receives one resolved private endpoint and the rest of the synchronization code remains unchanged. The explicit-address path remains a deterministic fallback.

### Platform strategy

USB networking is not identical across operating-system pairs:

- Android supports USB tethering to many desktop hosts; Android's own user documentation notes that USB tethering to Mac computers is not supported.
- Apple supports iPhone/iPad Personal Hotspot over USB on supported desktop platforms.
- Desktop and mobile operating systems vary in how the USB network interface is named, surfaced, permissioned, and activated.
- A future raw-USB path may be appropriate for combinations where the operating systems do not provide a usable point-to-point network bridge.

These differences are why platform detection/setup is intentionally kept out of the protocol. In particular, future iOS complexity should be isolated to the platform adapter that produces a duplex stream or resolved private endpoint; it must not leak into pairing, package, credential, or merge code.

No product USB workflow should require ADB, Android developer mode, or USB debugging. Those may be useful development diagnostics but are not part of the synchronization contract.

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

USB does not broaden this scope. A credential that would not be included in a LAN offer is not included merely because a cable is attached.

## Merge semantics

The receiver compares local and incoming rows with a stored common base. One-sided changes can apply, independent field changes can merge, and concurrent changes to the same field or delete-versus-edit cases become explicit conflicts. This is a versioned application merge policy, not a generic last-writer-wins database copy.

Managed-image identity collisions or byte/integrity disagreement fail rather than silently selecting an arbitrary file. The final merge validates the complete allowed graph and commits it transactionally. Related entities must remain consistent with foreign keys, schedule/history identities, and current lifecycle policy.

Removing trust deletes the relationship/baseline state according to the native policy, not cards or history already imported into the local learner's library. Sync is not remote revocation of another person's data.

## Cancellation and restart

Waiting, connecting, and transferring sessions can be canceled through the task/session signal and the transport-neutral abort handle. The cancellation registry stores no TCP socket. TCP implements abort by shutting down the socket; a future raw-USB backend can cancel/close its own native transfer through the same capability. Per-peer exclusivity prevents a second action racing the same baseline while one is active.

The short final database merge is intentionally non-cancellable. Interrupting it halfway would be a worse data contract than finishing one atomic commit. UI wording must distinguish a cancelable transfer from that final applying boundary.

Startup reconciles stale process-local sessions; old sockets, USB handles, and invitation acceptance callbacks do not survive process death. A retry uses actual current native state. The frontend must not make an obsolete completion overwrite a newer session's display.

## File fallback and backup distinctions

The supported file fallback exchanges the versioned study-data package and its validated managed assets, not a live SQL database mounted by both devices. Saved credentials remain excluded from that ordinary file path. Validate package version and scope rather than inferring that a filename is a safe full backup.

A `.studydeck` imports content with new local schedules. `.ostudybackup` replaces a coherent personal library through staged restart recovery. Sync merges its narrower allowed study graph using identities/common bases. These operations cannot be substituted without changing history/privacy semantics.

A USB cable does not turn a mounted phone directory or MTP folder into a shared database. File-copy USB fallback, if added later, must still exchange a versioned package through an explicit import boundary rather than opening the same SQLite file from two devices.

## Ownership, diagnostics, and tests

Native owners are `device_sync/mod.rs`, `store.rs`, `merge.rs`, `package.rs`, `transport.rs`, `crypto.rs`, `credentials.rs`, `network.rs`, and `cancellation.rs`. `transport.rs` owns transport-neutral framing; `network.rs` owns TCP-specific behavior. The frontend Settings component presents state and intent; platform adapters own OS-specific USB discovery/setup when implemented. None of those presentation/platform owners may acquire crypto or relational merge rules.

Tests cover protocol/identity framing, transport-neutral handshake framing, package/graph validation, field conflicts, baseline handling, managed-image integrity, credential opt-in, cancellation and overlap. Static checks enforce the stream/cancellation boundary, QR acquisition, stable port, and UI scope contracts. Android instrumentation checks native plugin routes; real peer/OEM/firewall/cable behavior remains explicit manual acceptance when not exercised.

Diagnostics report safe phase, endpoint/session outcome, and bounded errors without keys, invitation secrets, raw package contents, or personal source text. A storage/details screen identifies local diagnostic paths. Review exported diagnostics before sharing; metadata may still identify a device, model, interface, or filename.

# Trusted-device synchronization architecture

Synchronization is an explicit exchange between trusted ostudy installations. It is not a hosted account, automatic cloud backup, continuous background replication, shared SQLite file, or generic merge of every table in the application. [The user guide](../user-guide/device-sync.md) describes the actual workflow; [the comprehensive architecture](overview.md) describes the wider application data graph.

The synchronization architecture deliberately separates **sync semantics** from **transport mechanics**. Local-network TCP and direct USB are only ways to establish a duplex byte path; neither transport owns identity, cryptography, packages, conflict policy, credentials, or merge behavior.

## Layering and ownership

The synchronization subsystem has four conceptual layers:

1. **Sync orchestration and protocol** own one-way offers, session state, pairing transcripts, peer identity, acknowledgements, and progress/error transitions.
2. **Secure framing** owns handshake framing, authenticated encrypted frames, bounded control messages, package streaming, and file hashes.
3. **Transport adapters** own how bytes are read/written and how a blocked transfer is interrupted. `SyncIo` is the byte-stream boundary; `SyncAbort`/`SyncAbortSource` are the cancellation boundary.
4. **Package and merge services** own the explicit data allowlist, common-baseline comparison, conflicts, managed assets, credential scope, integrity validation, and atomic commit.

The core invariant is:

> A transport may establish, configure, read, write, close, or abort a connection. It must not acquire synchronization semantics.

`SecureChannel` therefore does not own `TcpStream`, and the cancellation registry does not own a TCP socket or a USB handle. TCP shutdown remains in `network.rs`; direct USB cancellation remains in `usb.rs`. Both hand the same `SyncIo` implementation to the protocol layer.

## Offer, receive, acknowledge

The sender creates a short-lived, one-way offer. How the receiver obtains that offer depends on transport:

- **LAN** renders the invitation as QR/text and advertises a private TCP endpoint.
- **Direct USB** sends the same kind of authenticated invitation as a bounded bootstrap message over the already attached USB accessory stream. The user never sees or transfers that invitation manually.

After the invitation is accepted, both paths execute the same client/server hello, authenticated key derivation, encrypted framing, package transfer, validation, merge, acknowledgement, and baseline update.

A reverse exchange is a separate explicit transfer from the other installation. Accepting one transfer never uploads the receiving device's library in reverse.

Only one unclaimed outgoing offer is retained; a newer outgoing action replaces the old one. Overlapping active sessions for the same peer are rejected.

## Protocol v3 and transport binding

Protocol v3 adds a typed transport to the ephemeral pairing invitation:

- `local-network` invitations contain a private IP/socket endpoint.
- `usb` invitations use only the symbolic endpoint `usb`.

The selected transport is part of the authenticated pairing transcript. A USB invitation cannot be relabeled as LAN without invalidating the transcript, and normal pasted/QR acceptance rejects a USB invitation. Conversely, Android accepts a direct-USB invitation only after it is received from the attached accessory descriptor.

This is intentionally separate from the synchronization **package** version. Package v3 remains the granular data-scope format; bumping the short-lived pairing protocol does not invalidate persistent peer identities, merge baselines, or package compatibility rules.

## Identity and authenticated channel

The native protocol uses stable Ed25519 identity, fresh X25519 exchange material, HKDF-derived session keys, ChaCha20-Poly1305 authenticated frames, and a one-time pairing secret. The invitation/session context binds the sender identity, ephemeral key, transport, endpoint, expiry, and relevant scope.

LAN carries the framed channel over private TCP. The listener uses **49251–49258**. Direct USB carries exactly the same framing over USB bulk I/O instead of an IP stack.

Physical cable presence is not treated as sufficient trust. USB retains the same identity verification, pairing proof, encryption, replay/expiry checks, package integrity validation, and merge policy as LAN.

## Direct USB architecture

### Why raw USB replaced USB networking

The original proof-of-concept reused TCP over an operating-system USB-tethering interface. That preserved protocol reuse but produced an unacceptable product workflow: users had to enable tethering, identify an adapter, discover an IP address, type it into ostudy, and potentially adjust firewall rules.

The maintained USB design removes USB networking entirely:

```text
                         synchronization protocol
                identity / crypto / package / merge
                                  |
                              SecureChannel
                                  |
                         SyncIo + SyncAbort
                           /              \
                        TCP               USB
                         |                 |
                  private LAN       Android Open Accessory
                                     bulk IN / bulk OUT
```

No TCP socket, IP address, tethering interface, QR code, or firewall rule exists on the cable path.

### Linux desktop host

Linux is currently the maintained desktop USB-host implementation. `device_sync/usb.rs` uses system `libusb-1.0` to implement Android Open Accessory (AOA) host mode.

The host:

1. discovers a compatible attached Android USB device;
2. queries AOA support with vendor request **51**;
3. supplies the oStudy accessory identity with request **52**;
4. starts accessory mode with request **53**;
5. waits for Android to re-enumerate as Google's AOA VID/PID (`18d1:2d00` or `18d1:2d01`);
6. claims the accessory interface and its bulk IN/OUT endpoints;
7. exposes those endpoints as `HostUsbStream: SyncIo + SyncAbortSource`.

The temporary session id is also advertised as the AOA serial identity so the re-enumerated accessory can be associated with the initiating host session when practical.

`libusb_bulk_transfer` uses short bounded wait slices. Cancellation does not require the synchronization core to know a libusb handle: `UsbAbort` flips the adapter's cancellation flag, and the transfer loop observes it between slices.

### Android accessory side

The private `ostudy-usb` Android plugin owns only Android platform mechanics:

- declares `android.hardware.usb.accessory` support;
- matches manufacturer `oStudy` and model `oStudy USB Sync`;
- receives `USB_ACCESSORY_ATTACHED`;
- opens the accessory through `UsbManager.openAccessory()`;
- detaches the `ParcelFileDescriptor` raw fd;
- hands that fd to Rust exactly once;
- emits `usb-sync-ready` so the React coordinator can claim an attachment that arrives while the app is already running.

Rust wraps that fd in `AndroidUsbStream`, using cancel-aware `poll/read/write`. Kotlin does not parse invitations, perform cryptography, inspect packages, or merge data.

The Android coordinator also performs an immediate claim when it mounts, covering the startup race where Android launched ostudy because the accessory was attached before the WebView listener existed.

### Cable bootstrap

After the Linux host opens the AOA stream, it sends one bounded plain framing message containing the short-lived protocol-v3 invitation. Android parses and validates it natively, confirms the transport is `usb`, and then enters the same authenticated client-hello flow used by LAN.

The bootstrap is not an unauthenticated data transfer. It only replaces QR/text delivery of the short-lived invitation; all study data still moves after the authenticated encrypted channel is established.

### Product UX and system integration

The intended product sequence is:

1. plug in one Android phone;
2. choose data scopes;
3. click **USB Sync**.

No product flow requires ADB, Android developer mode, USB debugging, USB tethering, an IP address, interface-name selection, or a firewall change.

Android may show its standard first-association chooser/permission prompt for the `oStudy USB Sync` accessory identity. That is an operating-system accessory trust decision, not developer authorization.

Linux needs permission to open the phone both before and after AOA re-enumeration. oStudy therefore owns a constrained udev rule covering the same supported Android OEM vendor IDs that the native adapter probes plus Google's AOA accessory IDs. Developer setup installs that rule under `/etc/udev/rules.d`; the Debian bundle installs it under `/usr/lib/udev/rules.d` and declares only the `libusb-1.0-0` runtime dependency. Product USB Sync does not depend on ADB or Android SDK packages. System USB permissions belong to installation/packaging, not to the synchronization screen.

The current direct-USB host implementation is Linux→Android. Other desktop/mobile combinations can add platform adapters behind `SyncIo` without changing pairing, encryption, packages, or merge code. Unsupported desktops report `usbHostAvailable = false` and do not show a fake/manual USB fallback.

## Android QR acquisition

QR acquisition remains a **LAN** feature. Android uses Google Code Scanner through Play services, not the retired ZXing path. The scanner owns its UI/camera permission; ostudy receives a bounded decoded result and validates its protocol natively. Pasted invitation text remains the LAN fallback when scanning is unavailable.

Direct USB does not use the camera scanner.

## Allowed data graph and sender-selected scopes

Synchronization never inherits every SQLite table automatically. The sender creates a package from an explicit, versioned selection of application-owned domains. Package v3 exposes five categories:

- **Flashcards** — deck folders/decks, notes/cards, tags, templates, managed card images, schedules, practice sessions/snapshots, review history, and portable source-citation records.
- **Documents** — canonical records, managed source/page/source-asset files, source units/blocks/assets, page and semantic/OCR-derived state, outlines/checkpoints, durable LLM generation jobs/units/windows/checkpoints, learning plans/topics, and document artifact metadata.
- **Study Guides** — guide revisions, sections, overlays, checkpoints, and guide artifact state. Study Guides require Documents.
- **API provider settings** — supported provider profile metadata and task defaults.
- **Saved API keys** — eligible secure-store credential bytes. Keys are separate opt-in data and require API provider settings.

Flashcards, Documents, and Study Guides default on. API provider settings and Saved API keys default off. Native validation rejects empty selections and invalid dependency combinations.

Cross-domain provenance is conditional. Document↔deck and document↔note/topic relationships move only when both Documents and Flashcards are selected.

The copied outgoing SQLite snapshot is **sanitized before transmission**. Unselected domains are removed. `app_settings` are always stripped. AI profile/probe owners sanitize their own transient or unselected state.

Managed document files reuse the complete-backup document-file ledger: expected managed paths, roles, byte sizes, and SHA-256 hashes are validated before transfer/install. Same-identity/different-byte collisions fail.

Package v3 records the granular selection explicitly while retaining read compatibility for the earlier package-v2 flashcard/API-settings shape.

## Optional API settings and credentials

**API provider settings** and **Saved API keys** are separate sender choices. Provider settings can move without credentials. Selecting Saved API keys requires provider settings.

Credential bytes never enter the SQLite snapshot, `.studydeck`, or `.ostudybackup`. They move only when explicitly selected, readable from sender secure storage, supported by receiver secure storage, and enabled by authenticated session metadata.

A credential-store failure after a successful learner-data merge reports a warning rather than rolling back the study-data merge. Lack of secure storage is never permission to write keys as plaintext.

USB does not broaden credential scope.

## Merge semantics

The receiver compares local and incoming rows with a stored common base. One-sided edits apply; independent field edits can merge; concurrent edits to the same field and delete-versus-edit cases become explicit conflicts.

Managed-image or managed-document-file identity/integrity disagreement fails rather than silently choosing arbitrary bytes. The final selected relational graph is validated and committed transactionally.

Removing trust removes peer/baseline state, not data already merged into the local library.

## Cancellation and restart

Waiting, connecting, and transferring sessions can be canceled through the session signal plus transport-neutral abort handle. TCP aborts by socket shutdown. USB adapters own their own cancel-aware I/O interruption.

The final **Merging** database commit is intentionally non-cancellable.

Startup reconciles stale process-local sessions because TCP sockets, USB descriptors, and accessory callbacks cannot survive process death.

## Backup/file boundaries

`.studydeck`, `.ostudybackup`, and live device sync are distinct operations. Device sync merges only sender-selected allowlisted domains using peer identity/common baselines. Backup performs staged coherent-library recovery; `.studydeck` imports portable deck content.

A USB cable never exposes or mounts the live SQLite database and does not turn MTP into a shared storage backend.

## Ownership, diagnostics, and tests

Native synchronization owners are `device_sync/mod.rs`, `store.rs`, `merge.rs`, `package.rs`, `scope.rs`, `transport.rs`, `crypto.rs`, `credentials.rs`, `network.rs`, `usb.rs`, and `cancellation.rs`.

Android platform USB mechanics live in `plugins/ostudy-platform/android/.../OstudyUsbPlugin.kt`; frontend attachment coordination lives in `UsbSyncCoordinator.tsx`. Neither owns cryptography or relational merge policy.

Static contracts assert that USB uses AOA requests/bulk I/O, both Linux and Android USB streams implement `SyncIo`, TCP remains isolated to the LAN adapter, manual USB-network UI does not reappear, Android accessory intent/filter wiring exists, and Linux packaging includes the USB runtime/rules integration without acquiring an ADB runtime dependency.

Unit/UI tests cover transport projection, sender scopes/dependencies, invitation transport confinement, package/privacy/merge rules, cancellation, and network QR behavior. Physical real-device cable acceptance remains human-required because CI cannot emulate a real USB controller, Android re-enumeration, cable quality, or host udev permissions.

Diagnostics report bounded phase/session/transport outcomes without keys, invitation secrets, raw package contents, or personal source text.

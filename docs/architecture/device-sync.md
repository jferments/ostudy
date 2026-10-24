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
3. choose **Send to a device**, then **USB cable** and **Send over USB**.

No product flow requires ADB, Android developer mode, USB debugging, USB tethering, an IP address, interface-name selection, or a firewall change.

Android may show its standard first-association chooser/permission prompt for the `oStudy USB Sync` accessory identity. That is an operating-system accessory trust decision, not developer authorization.

Linux needs permission to open the phone both before and after AOA re-enumeration. oStudy therefore owns a constrained udev rule covering the same supported Android OEM vendor IDs that the native adapter probes plus Google's AOA accessory IDs. Developer setup installs that rule under `/etc/udev/rules.d`; the Debian bundle installs it under `/usr/lib/udev/rules.d` and declares only the `libusb-1.0-0` runtime dependency. Product USB Sync does not depend on ADB or Android SDK packages. System USB permissions belong to installation/packaging, not to the synchronization screen.

The current direct-USB host implementation is Linux→Android. Other desktop/mobile combinations can add platform adapters behind `SyncIo` without changing pairing, encryption, packages, or merge code. Unsupported desktops report `usbHostAvailable = false` and do not show a fake/manual USB fallback.

## Android QR acquisition

QR acquisition remains a **LAN** feature. Android uses the bundled local JourneyApps ZXing Embedded/ZXing core scanner rather than Google Code Scanner or Play services. The platform adapter returns a bounded decoded result and ostudy validates its protocol natively. Pasted invitation text remains the LAN fallback when scanning is unavailable.

Direct USB does not use the camera scanner.

## Allowed data graph and sender-selected scopes

Synchronization never inherits every SQLite table automatically. The sender creates a package from an explicit, versioned selection of application-owned domains. Package v3 exposes seven categories:

- **Flashcards** — decks, notes/cards, collection copies retired into their originals (`card_merges`), tags, templates, managed card images, schedules, practice sessions/snapshots, review history with its scheduler context, the study-day totals kept after finalized deletions (`study_activity_retained`), and portable source-citation records.
- **Documents** — canonical records, study ranges, managed source/page/source-asset files, source units/blocks/assets, page and semantic/OCR-derived state, outlines/checkpoints, durable LLM generation jobs/units/windows/checkpoints, learning plans/topics, document artifact metadata, PDF page annotations (`document_annotations`), and the figures found in a PDF with their readings and the learner's choice to use its images (`document_figure_scans`, `document_figures`, `document_figure_passages`, `document_figure_readings`).
- **Study Guides** — guide revisions, sections, overlays, checkpoints, guide artifact state, how far the learner has read each guide (`document_study_guide_section_reads`, `document_study_guide_positions`), and the marks drawn on guide pages (`material_ink` rows with a `document_id`, synchronized as `guide_ink`). Study Guides require Documents.
- **Notes** — notebooks, their pages and marks, and their pictures (`notebooks`, `notebook_pages`, `notebook_items`, `notebook_item_assets`, and the managed images they hold, synchronized as `notebook_pictures`).
- **Lessons** — lessons, their revisions and the knowledge-unit hashes each was written from, lesson progress, teacher's lecture scripts, podcasts, each lesson's voice, saved voices, narration audio (`speech_segments`, `speech_voices`, and the managed audio they hold, synchronized as `lesson_audio`), and the marks drawn on slides (`material_ink` rows with a `lesson_id`, synchronized as `slide_ink`).
- **API provider settings** — supported provider profile metadata and task defaults.
- **Saved API keys** — eligible secure-store credential bytes. Keys are separate opt-in data and require API provider settings.

A table whose rows belong to different categories is synchronized in named parts, each with its own baselines and conflicts (`TableSpec::part`): `assets` is card images, `notebook_pictures` and `lesson_audio`, and `material_ink` is `guide_ink` and `slide_ink`. A part only ever reads its own rows, so a package without a category never looks like a deletion of that category's rows.

Subjects (`document_collections` and the document filing revision) travel with Flashcards, Documents, Notes or Lessons, so decks, documents, notebooks and lessons keep their `subject_id` with any of those categories alone; only a package with none of them removes them. The **Check these** list (`subject_filing_reviews`) is device-local and never travels.

Some learner state stays on the device that made it whatever the selection: the knowledge units ostudy extracted from documents, so a transferred lesson can be read and studied, but its source status on the receiving device is unavailable (`lesson_revision_units` names units that device does not have); the speech setting (`speech_settings`), which is a device preference; Tutor and shared-knowledge state; editor drafts; and the rebuildable per-deck review rollups (`review_day_rollups`). A whole-library backup is a snapshot of the library and keeps them.

Flashcards, Documents, Study Guides, Notes and Lessons default on. API provider settings and Saved API keys default off. A package from a build before Notes and Lessons travelled has no `notes` or `lessons` field in its selection, which reads as off. Native validation rejects empty selections and invalid dependency combinations.

Cross-domain provenance is conditional. Document↔deck and document↔note/topic relationships, and the link from a card to the PDF mark it was made from (`note_annotation_sources`), move only when both Documents and Flashcards are selected.

The copied outgoing SQLite snapshot is **sanitized before transmission**. Unselected domains are removed. `app_settings` are always stripped. AI profile/probe owners sanitize their own transient or unselected state.

Managed document files reuse the complete-backup document-file ledger: expected managed paths, roles, byte sizes, and SHA-256 hashes are validated before transfer/install. Same-identity/different-byte collisions fail.

Package v3 records the granular selection explicitly while retaining read compatibility for the earlier package-v2 flashcard/API-settings shape.

A package's `schemaVersion` is 30 plus the sender's release schema. Builds before release schema 6 recorded the frozen public number 30 and refuse any higher number with "Update this device first", so they refuse a package from an updated device before opening its snapshot. A receiver migrates a package recording its own number or less with `Database::open` before merging, so an updated device takes packages from older devices.

## Optional API settings and credentials

**API provider settings** and **Saved API keys** are separate sender choices. Provider settings can move without credentials. Selecting Saved API keys requires provider settings.

Credential bytes never enter the SQLite snapshot, `.studydeck`, or `.ostudybackup`. They move only when explicitly selected, readable from sender secure storage, supported by receiver secure storage, and enabled by authenticated session metadata.

A credential-store failure after a successful learner-data merge reports a warning rather than rolling back the study-data merge. Lack of secure storage is never permission to write keys as plaintext.

USB does not broaden credential scope.

## Merge semantics

The receiver compares local and incoming rows with a stored common base. One-sided edits apply; independent field edits can merge; concurrent edits to the same field and delete-versus-edit cases become explicit conflicts.

A card schedule is one FSRS state, so concurrent schedule changes keep the whole row with the later `last_review_at` (then `updated_at`, `revision` and device order) and record the other as a conflict instead of mixing fields. The merge also adds its own **Check these** entries: a deck or document the two devices filed in different subjects (`sync_filing_conflict`), and a collection copy that one device retired into its original while the other edited it (`sync_edited_retired_copy`). The copy stays retired with the edited text, so the edit can be found. A deck or document one device filed in a subject the other device deleted goes to No subject, as the deletion sent that subject's own decks and documents, and is listed as a `sync_filing_conflict` naming the deleted subject. A row that one device added or changed while the other deleted its parent follows the parent as deleting it on this device would have (`merge::PARENTS`): marks, figures and guide reading go with their document, pages and marks with their notebook, lessons and podcasts with their subject, and a lesson's revisions, progress, scripts, voice and slide marks with the lesson. A notebook whose subject was deleted goes to No subject instead, without a **Check these** entry. When both devices narrated the same text in the same voice, each keeps its own recording, since narration is kept once per text and voice. When the merge changes which documents a subject holds or their study ranges, it advances the document filing revision (`document_collection_organization`), which study materials are stamped with, even if both devices had already advanced it to the same number. See [ADR 0055](../decisions/0055-subjects.md).

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

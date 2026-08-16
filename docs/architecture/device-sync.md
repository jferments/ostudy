# Device synchronization architecture

ostudy synchronizes trusted installations without introducing a mandatory cloud account or copying a live SQLite database between devices.

## Scope

The synchronization snapshot contains durable study data from an explicit table allowlist:

- decks, folders, notes, generated card presentations, templates, tags, and their relationships;
- scheduling rows, immutable review events, and practice-session evidence;
- committed card-image metadata and bounded image payloads;
- ordinary application settings when the user enables that option;
- AI provider, endpoint, model, and task-default metadata when the user enables that option.

API keys and other credential bytes are not written into snapshots. They remain in the platform secure store. Device-only filesystem paths, Android permission state, window geometry, and temporary processing data are also outside the sync contract.

## Stable identity and common bases

Each installation receives a random 32-character device identity stored in app-private data. Every durable entity keeps its existing stable application identity. A trusted peer also has a saved acknowledged base snapshot.

A merge compares three states:

1. the last snapshot acknowledged for that peer;
2. the current local library;
3. the newly received remote snapshot.

This permits disjoint changes to merge without treating the newest whole database as authoritative.

## Merge behavior

Changes to different fields merge automatically. Examples include changing a deck description on one device while changing its name on another, or moving a card while another device edits its content.

Set-like relationship tables use stable composite identities. Independent tag additions, folder memberships, and note-asset relationships therefore union rather than replacing an entire array.

Immutable review events and practice sessions are unioned by their stable event identity. The history is never silently discarded because another device has a newer schedule row.

The following cases create explicit conflicts:

- both devices changed the same field from the same base;
- one device deleted a record while the other edited it;
- both devices independently created different values under the same stable identity;
- an append-only event identity refers to different immutable data.

The preview retains the local value, remote value, and common-base value. The user chooses a side or enters a custom text value before the merge is committed.

## Assets and crash safety

Card images are content-hashed, bounded per file and per snapshot, and validated before use. Changed assets are written to private staging files. They are promoted with reversible backups before the SQLite merge transaction commits. A failed database transaction restores the previous files; a failed file promotion never commits rows that reference unavailable bytes.

The acknowledged base is bookkeeping rather than user data. If writing that base fails after a successful database commit, ostudy records a diagnostic and reports the committed merge as successful instead of claiming that durable data was rolled back.

## QR pairing and LAN transport

A QR invitation identifies a short-lived one-shot private-LAN endpoint. The first pairing invitation also carries a random 256-bit pairing key. The snapshot is encrypted before it reaches the socket with ChaCha20-Poly1305 authenticated encryption. The HTTP endpoint therefore carries only ciphertext and accepts only the exact session path.

After the receiver authenticates and decrypts the snapshot, it returns an encrypted acknowledgement containing its device identity and the received snapshot identity. Both installations save the peer key in their platform secure store. Later QR sessions omit the key and require the existing trust record.

Endpoint validation permits only numeric private, link-local, or loopback addresses, requires an explicit port and exact session path, rejects URL credentials, query strings, fragments, and redirects, and expires sessions after a short interval.

The source device sends one snapshot per QR session. To exchange changes in both directions, either device can start the next session for the already trusted peer. A later protocol version can add simultaneous bidirectional exchange without changing the merge format.

## Android QR acquisition

Android can launch an installed camera application to capture the displayed QR code. The app-private image is decoded by ML Kit's QR barcode decoder and deleted immediately afterward. This barcode capability is separate from document OCR: PDF text recognition remains Tesseract-first.

Desktop users can paste the invitation text when no camera adapter is available.

## File fallback

The same versioned snapshot can be saved through the system document provider or shared through the platform share sheet. File transfer uses the same validation, common-base comparison, conflict preview, and commit logic as the encrypted LAN path. The file fallback intentionally excludes credentials and should be transferred through a channel the user trusts.

## Deletions and retired peers

Soft-deletion metadata participates in normal merging, preventing an older device from silently overwriting a known deletion. Removing a trusted peer deletes its transport key and saved common base. Long-term tombstone collection must account for every still-trusted peer before physically purging synchronization history.

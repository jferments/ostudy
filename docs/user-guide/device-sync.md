# Synchronize trusted devices

Device synchronization is local-first. ostudy does not require an account or upload the library to a central ostudy service.

## Pair and synchronize devices

1. Put both devices on the same private network.
2. Open **Settings → Device Sync** on the device that will send the first snapshot.
3. Give the installation a recognizable device name.
4. Leave **Transfer API credentials** disabled unless both devices are physically under your control and you intentionally want the destination secure store to receive those keys.
5. Select **Pair a new device** to create a short-lived QR invitation.
6. On Android, select **Scan QR with camera** on the receiving device. Desktop builds can paste the invitation text shown beside the QR code.
7. Let the encrypted transfer and merge complete, then review any conflicts that ostudy records under **Conflicts to resolve**.

The QR invitation expires after a short interval and contains a fresh random pairing secret plus the host's public identity and ephemeral key material. The library snapshot and any explicitly selected credentials are encrypted before they enter the local network.

The first successful session records the other installation as a trusted peer and saves the common synchronization baseline. Starting another **Pair a new device** session between the same installations creates a new short-lived QR/secret but recognizes the stable peer identities and uses the saved baseline for three-way merging. No long-lived transport secret is embedded in a synchronization file.

## Exchange changes in both directions

Protocol version 1 sends one current snapshot per QR session. To exchange offline changes in both directions:

1. Run a session from device A to device B.
2. Resolve any conflicts on B.
3. Start a new QR session from B and join it from A.
4. Resolve any remaining conflicts on A.

Because both installations retain the peer identity and common baseline, the return session normally merges only the remaining differences rather than treating either whole database as authoritative.

## What merges automatically

ostudy automatically combines independent work, including:

- cards created separately on two devices;
- different fields edited on the same deck or card;
- independent tag and folder relationships;
- immutable review events and practice sessions;
- image assets with distinct content hashes.

## What requires a decision

ostudy records an explicit conflict when:

- both devices changed the same field from the same common base;
- one device deleted an item while the other edited it;
- the same immutable event identity contains different data;
- both devices changed the same scheduling row.

Scheduling rows are treated atomically. ostudy never combines interval, due date, stability, difficulty, and revision values from two different devices into a schedule that existed on neither device. Immutable review history is retained even when a schedule conflict must be resolved.

Open **Conflicts to resolve**, choose the local or remote version (or the available merged resolution), and apply that decision explicitly. A conflict remains recorded until you resolve it.

## Settings and AI connections

The synchronization package includes ordinary application settings and AI connection-profile metadata supported by the current schema. Device-specific filesystem state, Android permissions, and temporary processing state remain outside the package.

AI connection-profile metadata can include provider type, endpoint, model ID, and task defaults. File snapshots never contain API keys.

API credentials can be transferred only when **Transfer API credentials** is explicitly enabled for the encrypted QR/LAN session. The destination writes accepted credential bytes through its secure-store boundary (Android Keystore on Android) rather than placing them in SQLite or a synchronization file.

A local endpoint such as `127.0.0.1` still refers to the destination device itself. A phone cannot reach a model running on a desktop through the desktop's loopback address; configure a private-network endpoint or authenticated proxy that the phone can reach.

## File fallback

When the devices cannot reach one another on the local network, use the synchronization file fallback in **Settings → Device Sync**. The file uses the same versioned snapshot and merge rules as LAN synchronization, but intentionally excludes every credential byte. Transfer it through a channel you trust and select the intended peer identity when importing it.

## Removing a trusted device

Select **Forget** beside the peer. ostudy removes that peer's trust record, synchronization baselines, conflicts, and related session records. Pair/synchronize again to establish a new common base.

Do not forget a device merely because it is temporarily offline. Deletion history must remain available to every still-trusted device so an old offline copy cannot silently overwrite known changes.

## Android PDF jobs while the app is backgrounded

A user-started PDF generation job enters an Android foreground service with a visible progress notification and a partial wake lock. Native PDF rendering, selective ML Kit OCR, text normalization, AI generation, and durable database commits can continue while the activity is not visible.

Android may still terminate the process under exceptional resource pressure or after a system restart. ostudy checkpoints document work durably; reopening the app resumes or retries from saved state instead of treating the WebView component as the owner of the job. Extremely large documents can still be more practical on a paired desktop, but ordinary PDF processing is not desktop-only.

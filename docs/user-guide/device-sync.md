# Synchronize trusted devices

Device synchronization is local-first. ostudy does not require an account or upload the library to a central ostudy service.

## Pair a new device

1. Put both devices on the same private network.
2. Open **Settings → Device Sync** on the device that will send the first snapshot.
3. Give the installation a recognizable device name.
4. Choose whether to include ordinary settings and AI connection-profile metadata.
5. Leave **Transfer API credentials** disabled unless both devices are physically under your control and you intentionally want the destination secure store to receive those keys.
6. Select **Pair a new device**.
7. On Android, select **Scan QR with camera** on the receiving device. Desktop builds can paste the invitation text shown beside the QR code.
8. Review the synchronization preview. Resolve every overlapping edit, then apply the synchronized changes.

The QR invitation expires after a short interval. First pairing includes a one-time random transport key inside the QR data. The library snapshot and any explicitly selected credentials are encrypted before they enter the local network.

## Synchronize an existing trusted device

After pairing, either device can select the trusted peer and choose **Start QR sync**. The new QR invitation does not contain the saved transport key; both devices load it from their platform secure stores.

A session sends one current snapshot. To exchange offline changes in both directions, run a session from device A to device B, resolve and commit it, then start a return session from device B to device A. Because the merge uses a saved common base, the return session normally contains only the remaining changes.

## What merges automatically

ostudy automatically combines independent work, including:

- cards created separately on two devices;
- different fields edited on the same deck or card;
- independent tag and folder relationships;
- immutable review events and practice sessions;
- image assets with distinct content hashes.

## What requires a decision

The preview asks for an explicit choice when:

- both devices changed the same field from the same common base;
- one device deleted an item while the other edited it;
- the same immutable event identity contains different data;
- both devices changed the same scheduling row.

Scheduling rows are treated atomically. ostudy never combines interval, due date, stability, difficulty, and revision values from two different devices into a schedule that existed on neither device. Immutable review history is retained even when a schedule conflict must be resolved.

## Settings and AI connections

Ordinary settings can be included. Device-specific paths, window geometry, Android permissions, and temporary processing state remain local.

AI connection-profile transfer includes provider type, endpoint, model ID, and task defaults. File snapshots never contain API keys.

API credentials can be transferred only through an explicitly enabled encrypted QR/LAN session. The destination keeps decrypted credentials in process memory until the merge is approved. Approval writes them into Android Keystore or the destination desktop secure store. Cancelling the preview destroys the staged records. If the app exits before approval, no credential is imported.

A local endpoint such as `127.0.0.1` still refers to the destination device itself. A phone cannot reach a model running on a desktop through the desktop's loopback address; configure the paired desktop's private-network endpoint or an authenticated proxy that the phone can reach.

## File fallback

When the devices cannot reach one another on the local network:

1. Select a trusted peer or enter its 32-character device identity.
2. Save or share a synchronization snapshot.
3. Transfer the file through a channel you trust.
4. On the destination, choose the snapshot and review the same conflict preview.

The file fallback uses the same merge rules and asset validation as QR synchronization, but it intentionally excludes every credential byte.

## Removing a trusted device

Select **Forget** beside the peer. ostudy removes the saved transport key and common synchronization base for that installation. Pair again to restore trust.

Do not forget a device merely because it is temporarily offline. Deletion history must remain available to every still-trusted device so an old offline copy cannot silently resurrect deleted study material.

## Android PDF jobs while the app is backgrounded

A user-started PDF generation job enters an Android foreground service with a visible progress notification and a partial wake lock. Rendering, Tesseract OCR, text normalization, AI generation, and durable database commits can continue while the activity is not visible.

Android may still terminate the process under exceptional resource pressure or after a system restart. ostudy checkpoints document work durably; reopening the app resumes or retries from saved state instead of treating the WebView component as the owner of the job. Extremely large documents can still be more practical on a paired desktop, but ordinary PDF processing is not desktop-only.

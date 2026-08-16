# Synchronize trusted devices

Device synchronization lets one ostudy installation send study data directly to another installation on the same private network. There is no ostudy cloud account involved.

Open **Settings → Backup/Sync → Device synchronization**.

## Send from this device

Use **Send to another device** on the device whose library changes you want to share.

1. Give the installation a recognizable device name if needed.
2. Leave **Include API provider settings and saved credentials** off unless you intentionally want to copy API configuration from this device.
3. Select **Create sync offer**.
4. Keep ostudy open while the short-lived QR code/invitation is waiting.
5. On the receiving device, scan the QR code or paste the invitation text.

The device that creates the offer is the **sender**. The receiving device does not need to generate a second invitation.

### Sending API settings

The API option is off by default for every offer.

When it is off, the transfer does not include provider profiles, API task defaults, or saved API credentials.

When the sender explicitly enables it, ostudy can also send the sender's AI-provider configuration. Saved credential bytes are included only when they are available from the sender's secure secret store and the receiver can store them securely.

This option does **not** include ordinary application settings. Theme, font, UI scale, motion, density/layout preferences, and other appearance/device preferences remain local to each installation.

## Receive on Android by scanning

On the receiving Android device:

1. Open **Settings → Backup/Sync → Device synchronization**.
2. Select **Scan and receive**.
3. Center the complete QR code inside the camera frame, including the white border.
4. Hold the phone steady until the scanner detects the code.

A successful QR scan is the acceptance gesture. ostudy closes the scanner and immediately connects to the sending device, authenticates the offer, receives the selected data, and merges it locally. There is no second **Pair and synchronize** confirmation and no reverse invitation to create.

If the scan is canceled, no synchronization starts.

## Receive from pasted invitation text

When camera scanning is unavailable, the sender can choose **Copy invitation text** and transfer that text through another trusted channel.

On the receiving device:

1. Paste it into **Synchronization invitation**.
2. Select **Accept sync offer**.

The direction is identical to QR pairing: the device that created the invitation sends, and the device that accepts it receives.

## What transfers

The standard study-library offer covers data needed to carry study material and learning state between installations, including decks/folders, cards, tags, templates, managed images, schedules, practice/history records, and review history.

General application/appearance settings do not transfer.

API provider configuration and saved credentials transfer only when the **sender explicitly enables** that option for the current offer.

## Sending changes back later

Accepting an offer is deliberately one-way. It does not give the sender permission to pull the receiver's library, and it does not start a permanent background sync relationship.

If you later want to send phone changes back to the desktop, create a **new sync offer on the phone** and accept it on the desktop. That reverse offer is a new intentional send action, not the second half of the original pairing.

Because paired devices retain stable identity and common merge baselines, later offers can distinguish ordinary one-sided edits from concurrent edits without requiring the receiving device to upload its library during the first acceptance.

## Conflicts

ostudy merges compatible edits automatically. When both devices changed the same field, or one deleted a record while the other edited it, ostudy records an explicit conflict instead of silently throwing away one side.

Open **Conflicts requiring a decision** to inspect the two versions and choose the desired result. Managed-image identity mismatches fail rather than picking arbitrary image bytes.

Appearance/theme settings never appear here in protocol v2 because they are excluded from synchronization. If you tested an earlier PR #51 build that created an `app_settings`/`uiScale` conflict, updating to protocol v2 retires that obsolete conflict while keeping the current device's local appearance value.

## Network requirements

Both devices must be able to reach each other on the same private network. Guest Wi-Fi/client isolation can prevent peer-to-peer connections even when both devices have internet access.

The sender normally detects its LAN address automatically. Use **Advanced LAN address override** only if needed, and enter the sender's private LAN IP rather than `127.0.0.1`.

For a local desktop AI service used from Android, remember that `127.0.0.1` on the phone means the phone itself; configure the provider with the desktop's reachable LAN address where appropriate.

## Invitation lifetime and compatibility

Offers expire after a short period. Create a fresh offer if one expires or the sending application is restarted.

Protocol-v1 invitations from earlier PR #51 test builds are intentionally not compatible with the corrected one-way protocol. After updating both installations, create a fresh protocol-v2 offer.

## Removing a trusted device

Removing a trusted device forgets its sync trust/baseline relationship. It does not delete decks/cards that already exist in your library.

A future transfer with that installation requires a new invitation.

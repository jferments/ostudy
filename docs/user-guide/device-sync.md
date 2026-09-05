# Synchronize trusted devices

Device synchronization lets one ostudy installation send study data directly to another trusted installation over either a private local network or a direct USB-network link. There is no ostudy cloud account involved.

Open **Settings → Backup/Sync → Device synchronization**.

## Choose the connection

The sender chooses how the receiving device should reach it:

- **Local network (Wi-Fi or Ethernet)** uses the sender's private LAN address. This remains the default and is normally detected automatically.
- **USB cable** uses a private point-to-point network carried by the USB cable. The synchronization protocol, encryption, pairing identity, package, merge rules, and conflicts are exactly the same as on Wi-Fi.

USB networking is supplied by the operating systems rather than by ADB. A product USB sync does **not** require Android developer mode or USB debugging.

## Send from this device

Use **Send to another device** on the device whose library changes you want to share.

1. Give the installation a recognizable device name if needed.
2. Choose **Local network (Wi-Fi or Ethernet)** or **USB cable** under **Connection**.
3. Leave **Include API provider settings and saved credentials** off unless you intentionally want to copy API configuration from this device.
4. Select **Create sync offer** (or **Create USB sync offer** for USB).
5. Keep ostudy open while the short-lived QR code/invitation is waiting.
6. On the receiving device, scan the QR code or paste the invitation text.

The device that creates the offer is the **sender**. The receiving device does not need to generate a second invitation.

Only one unclaimed outgoing offer is kept at a time. If you create a new offer while the previous QR is still waiting, ostudy cancels the old listener and replaces it. Once an offer has authenticated a particular peer, a different peer may still have a separate active transfer, but ostudy rejects overlapping active synchronization sessions with the same peer.

### Send over USB

The current USB backend uses the point-to-point network interface created by the operating system over the cable.

1. Connect the two devices with a **data-capable USB cable**.
2. Enable the operating system's USB networking feature. On Android this is normally **USB tethering**. On Apple devices this is provided through **Personal Hotspot over USB** on supported host platforms.
3. On the sending device, identify the private IP address assigned to its USB-network interface.
4. In ostudy choose **USB cable** and enter that address in **USB interface address**.
5. Create the USB sync offer.
6. Scan or paste the offer on the receiving device as usual.

The explicit USB-interface address is the deterministic fallback in this first implementation. We intentionally do not guess based on adapter names such as `rndis0`, `usb0`, `enx…`, or platform-specific labels. Automatic discovery belongs to the platform adapter so it can be improved independently without changing the synchronization protocol.

The QR/invitation does not become a different format merely because USB is selected. It contains the sender's private USB endpoint, so the receiver follows the cable's point-to-point network while using the same authenticated protocol-v2 session.

### USB platform notes

USB networking capabilities differ by host/mobile combination. Android supports USB tethering on many desktop hosts, but Android's own user documentation states that USB tethering to Mac computers is not supported. Apple supports iPhone/iPad Personal Hotspot over USB on supported desktop platforms. Other platform combinations depend on the operating system's available USB-network driver.

This difference is isolated below the sync protocol. A future native USB adapter can handle a combination that lacks a useful USB-network bridge without changing identity, crypto, packages, conflicts, credentials, or merge logic.

If a particular device pair cannot create a usable USB-network interface, use normal local-network sync for now rather than enabling developer mode or trying to expose an unauthenticated transport.

### Sending API settings

The API option is off by default for every offer.

When it is off, the transfer does not include provider profiles, API task defaults, or saved API credentials.

When the sender explicitly enables it, ostudy can also send the sender's AI-provider configuration. Saved credential bytes are included only when they are available from the sender's secure secret store and the receiver can store them securely.

This option does **not** include ordinary application settings. Theme, font, UI scale, motion, density/layout preferences, and other appearance/device preferences remain local to each installation.

USB does not change this scope. Connecting a cable never automatically authorizes credential transfer.

## Receive on Android by scanning

On the receiving Android device:

1. Open **Settings → Backup/Sync → Device synchronization**.
2. Select **Scan and receive**.
3. Center the complete QR code inside the camera frame, including the white border.
4. Hold the phone steady until the scanner detects the code.

A successful QR scan is the acceptance gesture. ostudy closes the scanner and immediately connects to the sending device, authenticates the offer, receives the selected data, and merges it locally. There is no second **Pair and synchronize** confirmation and no reverse invitation to create.

The receive workflow is transport-independent. For a USB offer, the invitation endpoint points at the sender's private USB-network address; for an ordinary LAN offer it points at the sender's LAN address.

If the initial connection cannot be completed, the scanned invitation remains loaded and the action changes to **Retry connection**. You do not need to rescan while that offer remains valid.

If the scan is canceled, no synchronization starts.

## Receive from pasted invitation text

When camera scanning is unavailable, the sender can choose **Copy invitation text** and transfer that text through another trusted channel.

On the receiving device:

1. Paste it into **Synchronization invitation**.
2. Select **Accept sync offer**.

The direction is identical to QR pairing: the device that created the invitation sends, and the device that accepts it receives.

## Canceling a request

Active requests appear beneath the send/receive panels with their direction, current phase, peer name when known, and a **Cancel** action.

Waiting, connecting, and transferring sessions can be canceled. Cancellation invokes the active transport's abort capability as well as marking the durable session canceled, so a stalled network or future native-USB request does not remain displayed as active. The database merge itself is an atomic commit boundary; once a receiver has entered the final **Merging** phase, ostudy lets that short commit finish instead of interrupting it halfway through.

If ostudy is restarted while a request is still recorded as waiting/connecting/transferring/merging, startup marks that old request canceled/interrupted because its process-local transport cannot have survived the restart.

## What transfers

The standard study-library offer covers data needed to carry study material and learning state between installations, including decks/folders, cards, tags, templates, managed images, schedules, practice/history records, and review history.

General application/appearance settings do not transfer.

API provider configuration and saved credentials transfer only when the **sender explicitly enables** that option for the current offer.

## Sending changes back later

Accepting an offer is deliberately one-way. It does not give the sender permission to pull the receiver's library, and it does not start a permanent background sync relationship.

If you later want to send phone changes back to the desktop, create a **new sync offer on the phone** and accept it on the desktop. That reverse offer is a new intentional send action, not the second half of the original pairing.

The reverse offer can use a different transport from the previous transfer. For example, you can pair/sync desktop → phone over USB and later send phone → desktop over Wi-Fi. Stable device identity and common merge baselines belong to the installations, not to the cable or network route.

Because paired devices retain stable identity and common merge baselines, later offers can distinguish ordinary one-sided edits from concurrent edits without requiring the receiving device to upload its library during the first acceptance.

## Conflicts

ostudy merges compatible edits automatically. When both devices changed the same field, or one deleted a record while the other edited it, ostudy records an explicit conflict instead of silently throwing away one side.

Open **Conflicts requiring a decision** to inspect the two versions and choose the desired result. Managed-image identity mismatches fail rather than picking arbitrary image bytes.

Appearance/theme settings never appear here in protocol v2 because they are excluded from synchronization. If you tested an earlier PR #51 build that created an `app_settings`/`uiScale` conflict, updating to protocol v2 retires that obsolete conflict while keeping the current device's local appearance value.

## Local-network requirements

For ordinary LAN sync, both devices must be able to reach each other on the same private network. Guest Wi-Fi/client isolation can prevent peer-to-peer connections even when both devices have internet access.

The sender normally detects its LAN address automatically. Use **Advanced LAN address override** only if needed, and enter the sender's private LAN IP rather than `127.0.0.1`.

Outgoing offers listen on the small, stable TCP range **49251–49258** instead of a new random port every time. This allows a desktop firewall rule to be configured once while still leaving enough listener slots for separate peers to synchronize concurrently. If a receiver reports that the QR was read but the sender could not be reached, verify that the sender is actually listening on the displayed endpoint and permit inbound TCP 49251–49258 from the relevant private interface/subnet.

On Linux with UFW, for example, inspect the firewall with `sudo ufw status verbose`. If it is active, an appropriately scoped rule can permit the ostudy range, such as `sudo ufw allow from <phone-or-private-subnet> to any port 49251:49258 proto tcp`. Adapt the source scope to your network rather than opening the range to the public internet.

For USB-network sync, apply any firewall allowance only to the private USB interface/subnet where possible. The USB cable does not disable the host firewall.

A successful transfer in the opposite direction does not by itself prove this direction is open: the host/listener is the device that created the current offer, so reversing which device creates the offer reverses which device must accept an inbound TCP connection.

For a local desktop AI service used from Android, remember that `127.0.0.1` on the phone means the phone itself; configure the provider with the desktop's reachable private address where appropriate.

## Diagnostics

Native diagnostic events are stored as JSON Lines in the app-private `diagnostics/ostudy.jsonl` file. The exact path for the current installation is shown at **Settings → Backup/Sync → Storage → Diagnostics**. The file rotates at a bounded size, retaining the previous segment as `ostudy.previous.jsonl`.

Database failures keep their detailed SQLite error out of the ordinary UI, but the diagnostic event includes the raw SQLite message plus its native source call site. Device synchronization also records major phases such as offer creation, connection, authentication, package receipt, merge start, and merge completion so a failed physical-device transfer can be localized without guessing from a generic error message.

On a debuggable Android development installation connected over USB, `adb` can still be used to inspect app-private diagnostics. That development/debugging path is unrelated to product USB synchronization and is not required to use USB sync.

## Invitation lifetime and compatibility

Offers expire after a short period. Create a fresh offer if one expires or the sending application is restarted.

Protocol-v1 invitations from earlier PR #51 test builds are intentionally not compatible with the corrected one-way protocol. After updating both installations, create a fresh protocol-v2 offer.

LAN and USB-network offers use the same protocol-v2 invitation and peer identity. Switching transports does not require forgetting/re-pairing a trusted device.

## Removing a trusted device

Removing a trusted device forgets its sync trust/baseline relationship. It does not delete decks/cards that already exist in your library.

A future transfer with that installation requires a new invitation.

## Android QR scanner backend

Android pairing uses Google Code Scanner. The scanner UI and camera permission are owned by Google Play services; ostudy itself does not request the camera permission. The scanner accepts QR codes only and returns the decoded pairing envelope to ostudy for protocol and cryptographic validation. Devices without a compatible Google Play services scanner module receive an unavailable-scanner message and can use the manual pairing path.

# Synchronize trusted devices

Device synchronization lets one ostudy installation send selected data directly to another trusted installation over a private local network or a direct USB-network link. There is no ostudy cloud account and accepting an offer never uploads the receiving device's library back to the sender.

Open **Settings → Backup/Sync → Device synchronization**.

## Choose the connection

The sender chooses how the receiving device reaches it:

- **Local network (Wi-Fi or Ethernet)** uses the sender's private LAN address. This is the default and ostudy normally detects the address automatically.
- **USB cable** uses a private point-to-point network carried over the USB cable. Pairing identity, authentication, encryption, package validation, merge rules, conflicts, cancellation, and the selected data scopes are the same as on Wi-Fi.

USB networking is supplied by the operating systems rather than by ADB. Product USB sync does **not** require Android developer mode or USB debugging.

## Choose what to synchronize

Every outgoing offer has a **What to synchronize** section. The sender chooses the categories included in that one transfer:

- **Flashcards** — decks, folders, notes/cards, templates, tags, schedules, practice/review history, source-citation snapshots, and managed card images.
- **Documents** — original managed source files plus canonical document content, rendered pages/source assets, OCR and semantic extraction data, outlines, durable LLM generation jobs/units/windows/checkpoints, learning plans/topics, and other document-processing metadata needed to continue working with the document on the receiving device.
- **Study Guides** — generated study guides, sections, overlays, checkpoints, and related study-guide artifact state.
- **API provider settings** — AI provider profiles and task defaults. Ordinary app preferences are not included.
- **Saved API keys** — securely stored provider credentials. Key bytes are a separate opt-in from provider settings.

Flashcards, Documents, and Study Guides are selected by default. **API provider settings** and **Saved API keys** are off by default.

Some categories have required dependencies. Selecting **Study Guides** also selects **Documents**, because a study guide belongs to its source-document graph. Selecting **Saved API keys** also selects **API provider settings**, because credentials have meaning only for their provider profiles. If the parent category is turned off, its dependent category is turned off too. These rules are validated again by the native backend; they do not rely only on checkbox state.

At least one transferable category must be selected before an offer can be created.

### Privacy of unselected categories

An unselected category is removed from the copied synchronization snapshot before the package is transmitted. It is not merely ignored during merge. This matters especially for API configuration and documents: data that the sender did not choose should not hitchhike inside the encrypted package.

General application and appearance settings always stay local. Theme, font, UI scale, motion, density/layout preferences, and similar device preferences are not part of device sync.

Transient AI probe evidence is also not transferred. AI-owned repositories sanitize provider/probe state before the copied snapshot becomes transfer material.

## Send from this device

Use **Send to another device** on the device whose changes you want to share.

1. Give the installation a recognizable device name if needed.
2. Choose **Local network (Wi-Fi or Ethernet)** or **USB cable** under **Connection**.
3. Under **What to synchronize**, check the categories to include in this offer.
4. Select **Create sync offer** or **Create USB sync offer**.
5. Keep ostudy open while the short-lived QR code/invitation is waiting.
6. On the receiving device, scan the QR code or paste the invitation text.

The device that creates the offer is the **sender**. The receiving device does not generate a second invitation.

Only one unclaimed outgoing offer is kept at a time. Creating a new offer while an older QR code is still waiting cancels and replaces the old listener. ostudy also prevents overlapping active synchronization sessions with the same peer.

## Send over USB

The current USB backend uses the point-to-point network interface created by the operating system over the cable.

1. Connect the two devices with a **data-capable USB cable**.
2. Enable the operating system's USB networking feature. On Android this is normally **USB tethering**. On Apple devices it may be provided through **Personal Hotspot over USB** on supported host platforms.
3. On the sending device, identify the private IP address assigned to the USB-network interface.
4. In ostudy choose **USB cable** and enter that address in **USB interface address**.
5. Choose the same data categories you would choose for a Wi-Fi transfer.
6. Create the USB sync offer and scan or paste it on the receiving device.

The explicit USB-interface address is the deterministic fallback in this implementation. ostudy does not guess from adapter names such as `rndis0`, `usb0`, `enx…`, or platform-specific labels. Automatic discovery can be improved later inside a platform adapter without changing synchronization protocol or merge semantics.

The invitation format does not change for USB. It contains the sender's private USB-network endpoint, so the receiver follows that point-to-point route while using the same authenticated protocol-v2 session.

### USB platform notes

USB networking capabilities differ by host/mobile combination. Android USB tethering works with many desktop hosts, while some platform combinations have OS-level limitations. Apple devices expose USB networking only on supported host/platform combinations.

If a device pair cannot create a usable USB-network interface, use normal local-network sync. Do not enable developer mode or expose an unauthenticated transport merely to make product sync work.

The USB cable also does not bypass the host firewall. Scope any firewall allowance to the private USB interface/subnet when possible.

## API provider settings and saved API keys

These are intentionally two different checkboxes.

Selecting **API provider settings** transfers provider profiles and task-default configuration, but does not by itself authorize saved key bytes. Selecting **Saved API keys** opts into credentials as well and automatically selects provider settings.

Credential bytes are sent only when all of the following are true:

- **Saved API keys** was selected for the outgoing offer.
- The sender can read the credential from its secure secret store.
- The receiver reports usable secure credential storage.
- The authenticated session metadata agrees that credential transfer is enabled.

If the study data transfer succeeds but a credential cannot be stored securely on the receiver, ostudy keeps the successful library merge and reports a credential warning rather than discarding the study data.

Connecting a USB cable never automatically enables API configuration or credentials.

## Receive on Android by scanning

On the receiving Android device:

1. Open **Settings → Backup/Sync → Device synchronization**.
2. Select **Scan and receive**.
3. Center the complete QR code inside the camera frame, including the white border.
4. Hold the phone steady until the scanner detects it.

A successful QR scan is the acceptance gesture. ostudy closes the scanner and immediately connects to the sender, authenticates the offer, receives the sender-selected package, validates it, and merges it locally. There is no second **Pair and synchronize** confirmation and no reverse invitation.

The receiver does not get a second set of scope checkboxes for that incoming offer. The sender's authenticated offer/package defines what is being sent. To request a different set of data, create a new offer on the sending device with different selections.

If the initial connection cannot be completed, the scanned invitation remains loaded and the action changes to **Retry connection**. You do not need to rescan while the offer remains valid.

Canceling the scanner does not start synchronization.

## Receive from pasted invitation text

When camera scanning is unavailable, the sender can choose **Copy invitation text** and transfer it through another trusted channel.

On the receiving device:

1. Paste it into **Synchronization invitation**.
2. Select **Accept sync offer**.

The direction is identical to QR pairing: the installation that created the invitation sends, and the installation that accepts it receives.

## Canceling a request

Active requests appear beneath the send/receive panels with their direction, current phase, peer name when known, and a **Cancel** action.

Waiting, connecting, and transferring sessions can be canceled. Cancellation invokes the active transport's abort capability as well as marking the durable session canceled, so cancellation behavior is not tied to TCP specifically.

The database merge is an atomic commit boundary. Once the receiver enters the final **Merging** phase, ostudy lets that short commit finish instead of interrupting the database halfway through.

If ostudy restarts while a request is still recorded as waiting/connecting/transferring/merging, startup marks the old request interrupted because its process-local transport cannot have survived the restart.

## Documents and their LLM processing data

Selecting **Documents** transfers more than the visible source file. The package includes the managed document files and the durable relational graph needed for the receiving device to retain document-processing progress: canonical source units/blocks/assets, page and semantic data, outlines and outline checkpoints, generation jobs and their units/windows/checkpoints, learning plans/topics, and document artifact metadata.

This lets the receiving installation continue from the synchronized document state instead of behaving as though the source file had never been processed.

Document files use the same validated file-ledger rules as library backup: managed relative paths, sizes, hashes, and expected file roles are checked before installation. A local file with the same managed identity but different bytes is treated as an identity conflict rather than silently overwritten.

When **Documents** is selected but **Flashcards** is not, document processing remains transferable while generated-deck/card relationships are detached from the package. When both are selected, document↔flashcard provenance links can travel with the two graphs.

## Study Guides

Study Guides are a separate checkbox so a user can synchronize Documents without necessarily copying generated study-guide artifacts. Because guides reference document learning topics and source-document state, **Study Guides** requires **Documents**.

Turning Study Guides off while leaving Documents on removes the study-guide rows and study-guide artifact jobs from the copied snapshot before transmission.

## Sending changes back later

Accepting an offer is deliberately one-way. It does not give the sender permission to pull the receiver's library and does not establish permanent background replication.

To send phone changes back to a desktop, create a **new sync offer on the phone** and accept it on the desktop. That reverse offer can choose a different set of categories and can use a different transport. For example, desktop → phone may use USB while phone → desktop later uses Wi-Fi.

Stable device identity and merge baselines belong to the installations, not to the cable or network route. Later transfers can therefore distinguish ordinary one-sided changes from concurrent edits.

## Conflicts

ostudy merges compatible edits automatically. When both devices changed the same synchronized field, or one deleted a record while the other edited it, ostudy records an explicit conflict instead of silently throwing away one side.

Open **Conflicts requiring a decision** to inspect the two versions and choose the desired result. Managed image or document-file identity mismatches fail rather than selecting arbitrary bytes.

Appearance/theme settings never participate in synchronization conflicts because they are excluded from the transfer and merge allowlists.

## Local-network requirements

For LAN sync, both devices must be able to reach each other on the same private network. Guest Wi-Fi/client isolation can prevent peer-to-peer connections even when both devices have internet access.

The sender normally detects its LAN address automatically. Use **Advanced LAN address override** only when needed, and enter the sender's private LAN IP rather than `127.0.0.1`.

Outgoing offers listen on TCP **49251–49258**. If a receiver reads the QR but cannot reach the sender, verify that the sender is listening on the displayed endpoint and that the host firewall permits the relevant private interface/subnet.

On Linux with UFW, inspect the firewall with `sudo ufw status verbose`. If needed, use a source-scoped rule for the private peer/subnet rather than opening the sync range to the public internet.

A successful transfer in the opposite direction does not prove the current direction is open: the listener is always the device that created the current offer.

For a local desktop AI service used from Android, remember that `127.0.0.1` on the phone means the phone itself. Configure a reachable private desktop address where appropriate.

## Diagnostics

Native diagnostic events are stored as JSON Lines in the app-private `diagnostics/ostudy.jsonl` file. The exact path is shown at **Settings → Backup/Sync → Storage → Diagnostics**. The file rotates at a bounded size, retaining the previous segment as `ostudy.previous.jsonl`.

Device synchronization records major phases such as offer creation, connection, authentication, package receipt, merge start, and merge completion. Detailed SQLite errors remain out of ordinary user-facing messages but are preserved in diagnostics with their native source location.

On a debuggable Android development build, `adb` can still be used to inspect app-private diagnostics. That debugging path is unrelated to product USB synchronization and is not required for USB sync.

## Invitation lifetime and compatibility

Offers expire after a short period. Create a fresh offer if one expires or the sender application is restarted.

Protocol-v1 invitations from early test builds are intentionally not compatible with the corrected one-way protocol. LAN and USB-network offers use the same protocol-v2 invitation and peer identity.

The synchronization package has its own versioned format. New granular-scope packages record the selected categories explicitly while retaining compatibility handling for the earlier flashcard/API-settings package format.

## Removing a trusted device

Removing a trusted device forgets its synchronization trust/baseline relationship. It does not delete study data already merged into the local installation.

A future transfer with that installation requires a new invitation.

## Android QR scanner backend

Android pairing uses Google Code Scanner. The scanner UI and camera permission are owned by Google Play services; ostudy itself does not request camera permission. The scanner accepts QR codes only and returns the decoded pairing envelope to ostudy for protocol and cryptographic validation. Devices without a compatible Google Play services scanner module can use the pasted-invitation path.

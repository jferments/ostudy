# Synchronize trusted devices

Device synchronization lets one ostudy installation send selected data directly to another trusted installation. There is no ostudy cloud account, and accepting a transfer never uploads the receiving device's library back to the sender.

Open **Settings → Backup and sync**. **Devices** lists this device and the devices it has synchronized with; **Send to a device** and **Receive** start a transfer, and **Rename** in this device's menu changes the name other devices see.

## Two connection paths

ostudy has two transport adapters under the same authenticated synchronization protocol:

- **USB Sync** is the preferred cable workflow on a supported Linux desktop sending to Android. Plug in one Android phone with a data-capable USB cable, choose **Send to a device**, pick **USB cable** under **How to send**, choose the data, and choose **Send over USB**. There is no USB tethering, IP address, network-adapter selection, QR scan, ADB, developer-mode, or USB-debugging setup in the product workflow.
- **Local network** uses Wi-Fi or Ethernet. The sender shows a short-lived sync code, as a QR code or text, containing its private LAN endpoint; the receiver scans or pastes it.

The transport changes only how the two native processes exchange bytes. Device identity, pairing proof, authenticated key exchange, encryption, package validation, selected data scopes, merge rules, conflicts, typed-question admission, and cancellation are shared.

## Choose what to synchronize

Every outgoing transfer has a **What to send** section. The sender chooses the categories included in that transfer:

- **Flashcards** — decks, Basic/Fill in the blank and native interactive notes/cards, stable interactive presentation identities, templates, tags, schedules, retained practice/review evidence, source-citation snapshots, and managed card/question images.
- **Documents** — original managed source files plus canonical document content, rendered pages/source assets, OCR and semantic extraction data, outlines, durable LLM generation jobs/units/windows/checkpoints, generation reports, learning plans/topics, and document-processing metadata needed to continue working with the document on the receiving device, with your PDF annotations and the figures found in your PDFs.
- **Study guides** — generated study guides, sections, overlays, checkpoints, and related guide artifact state, how far you have read each guide, and what you drew on guide pages.
- **Notes** — your notebooks with their pages, everything drawn or written on them, and their pictures. Notes folders and pins stay on each device: a synced notebook arrives at the top of Notes, unpinned.
- **Lessons** — your lessons and how far you are through each, teacher's lectures, podcasts, the voice each lesson is read in, your saved voices, narration audio, and what you drew on slides.
- **AI services** — AI service profiles and task defaults. Ordinary app preferences are not included.
- **Saved API keys** — securely stored provider credentials. Key bytes are a separate opt-in from AI services.

Subjects, which hold decks, documents, notebooks and lessons, travel with any of **Flashcards**, **Documents**, **Notes** or **Lessons**, so each keeps its subject on the other device. If one device deletes a subject while the other files a deck, document or notebook in it, that item ends up in No subject, the way deleting a subject leaves the decks and documents it held. A lesson or podcast in a deleted subject is deleted with it, as deleting the subject on this device would. In the same way, marks added on one device to a document, notebook or lesson that the other device deleted go with it.

Flashcards, Documents, Study guides, Notes and Lessons are selected by default. **AI services** and **Saved API keys** are off by default. **Saved API keys** is unavailable when this device has no secure key storage.

Selecting **Study guides** also selects **Documents**. Selecting **Saved API keys** also selects **AI services**. Turning a required parent category off turns its dependent category off too. Native validation repeats these rules; they do not rely only on switch state.

At least one transferable category must be selected.

### Privacy of unselected categories

An unselected category is removed from the copied synchronization snapshot before transmission. It is not merely ignored during merge.

When Flashcards is unselected, typed question definitions, schedules, attempts, question-review links, remediation records and managed card assets are removed with that graph. Private editor drafts, question write receipts, asset-reconciliation diagnostics and other local operation records are excluded even when Flashcards is selected.

Some data always stays on the device, whatever is selected:

- General application and appearance settings: theme, font, UI scale, motion, layout and similar preferences.
- Tutor projects, conversations, summaries and their search copies (see [Tutor memory and privacy](tutor-memory-and-privacy.md)).
- The knowledge ostudy extracted from documents, so a transferred document may be read again before new material is made from it on the receiving device. A transferred lesson can be studied straight away, but on the receiving device it cannot check whether its documents changed since it was written.
- Your choice of speech model and voice in Settings.
- How generated questions were made (their generation evidence) and your **Question mix** in **Settings → Card templates**.
- AI model test results.

Use a whole-library backup to move these.

## Direct USB: plug in, select, sync

For a supported Linux desktop sending to Android:

1. Open ostudy on the desktop and Android phone.
2. Connect **one Android phone** with a data-capable USB cable.
3. On the desktop, choose **Send to a device**, pick **USB cable** under **How to send**, and select the categories under **What to send**.
4. Choose **Send over USB**.
5. Keep both devices connected until ostudy reports completion.

That is the normal cable workflow. You do **not** enable USB tethering, locate a USB interface, enter an IP address, change firewall rules, scan a QR code, enable Android developer options, or enable USB debugging.

On the first accessory association, Android can display a system prompt asking which app should handle the attached **oStudy USB Sync** accessory. Choose ostudy. That operating-system trust prompt is distinct from developer-mode/ADB authorization and is normally a one-time association for the accessory identity.

The Linux app uses Android Open Accessory (AOA) host mode. The desktop switches the attached Android device into the oStudy accessory identity and communicates over the accessory's USB bulk endpoints. Android opens that accessory through `UsbManager`; the native plugin hands its file descriptor to the shared Rust synchronization layer. No IP stack exists on the cable path.

If no compatible phone is found, multiple phones are attached, Linux denies the USB device, or the cable is disconnected, ostudy reports that condition directly. The Linux installer/developer setup installs the system USB-access rules needed by the app; users are not expected to run USB networking commands.

### Direction of a USB transfer

The desktop **Send over USB** action creates the outgoing transfer and sends the selected data to the attached Android phone. The Android app accepts the cable invitation automatically from the accessory stream.

To send phone changes back later, create a new outgoing transfer on the phone using the available supported sender transport, such as local-network sync. Device identity and merge baselines belong to the installations, not to the transport.

## Send over the local network

For Wi-Fi/Ethernet:

1. On the sending device, choose **Send to a device** and select the categories under **What to send**.
2. Choose **Show a code**.
3. Keep ostudy open while the short-lived code is waiting.
4. On the receiving device, choose **Receive** and then **Scan the code**, or use **Copy as text** on the sender and paste the code through another trusted channel.

Only one unclaimed outgoing offer is retained at a time. **Make a new code** cancels the previous unclaimed listener. ostudy also prevents overlapping active synchronization sessions with the same peer.

The sender normally detects its LAN address automatically. **Network address**, under **More options**, exists only for unusual LAN routing problems and is never part of USB Sync.

## Receive a local-network offer on Android

On the receiving Android device:

1. Open **Settings → Backup and sync**.
2. Choose **Receive**, then **Scan the code**.
3. Grant camera access if Android asks for it.
4. Center the complete QR code inside the camera frame, including the white border.
5. Hold the phone steady until the scanner detects it.

A successful QR scan is the acceptance gesture. ostudy immediately connects to the sender, authenticates the offer, receives the sender-selected package, validates it, and merges it locally. There is no reverse invitation.

If the initial network connection fails, the scanned invitation remains loaded and the button changes to **Try again** while the offer is still valid. Canceling the scanner starts nothing.

When camera scanning is unavailable or permission is declined, paste the text from the sender's **Copy as text** into **Paste the code as text** and choose **Receive**.

Direct USB does not use either of these QR/text receive controls; Android receives the cable bootstrap from the native accessory descriptor automatically.

## API provider settings and saved API keys

These are intentionally separate choices. Selecting **AI services** transfers service profiles and task defaults but does not authorize key bytes. Selecting **Saved API keys** opts into credentials and automatically selects **AI services**.

Credential bytes are sent only when all of the following are true:

- **Saved API keys** was selected for the outgoing transfer.
- The sender can read the credential from its secure secret store.
- The receiver reports usable secure credential storage.
- The authenticated session metadata agrees that credential transfer is enabled.

If study data merges successfully but a credential cannot be stored securely on the receiver, ostudy keeps the successful learner-data merge and reports a credential warning. Connecting a cable never automatically enables API settings or credentials.

## Canceling a request

Transfers in progress show their direction, current phase, peer name when known, and a **Cancel** action.

Waiting, connecting, and transferring sessions can be canceled. Cancellation invokes the active transport's abort capability as well as marking the durable session canceled, so the same behavior works for TCP and USB.

The final **Merging** phase is an atomic database commit boundary. Once that short commit begins, ostudy lets it finish rather than interrupting the database halfway through.

If ostudy restarts while a request is still recorded as active, startup marks the old request interrupted because its process-local TCP socket or USB descriptor cannot survive the process restart.

## Typed interactive questions

When **Flashcards** is selected, synchronization carries the canonical typed question definition and stable presentation ownership for single-best-answer, select-all, true/false, matching, grouping, ordering, and image occlusion. Managed assets referenced by those definitions travel through the same validated asset path.

Retained first-attempt checkpoints, question/review links and remediation records also travel. The receiver checks question definitions, their presentations, their images, attempt checkpoints and review links before applying anything. When both devices changed the same attempt or checkpoint, the whole record is kept from one side and the other is recorded as a conflict, never mixed field by field.

If a received graph is malformed or its typed ownership no longer agrees, the package is rejected before files are installed or the merge commits. Sync does not silently flatten an interactive question into Basic or discard its presentation identities.

## Documents and their LLM processing data

Selecting **Documents** transfers the managed source files and durable relational graph required to retain processing progress: canonical source units/blocks/assets, page and semantic data, outlines/checkpoints, generation jobs and their units/windows/checkpoints/reports, learning plans/topics, and document artifact metadata.

Document files use the same validated file-ledger rules as complete library backup: managed relative paths, sizes, hashes, and expected file roles are checked before installation. A local file with the same managed identity but different bytes fails rather than being silently overwritten.

When **Documents** is selected but **Flashcards** is not, document processing remains transferable while document↔generated-card relationships are detached from the package. When both are selected, supported provenance links can travel with both graphs.

## Study guides

Study guides are a separate category so Documents can be synchronized without generated guide artifacts. Because guide identity depends on document learning topics and source-document state, **Study guides** requires **Documents**.

Turning Study guides off while leaving Documents on removes guide rows and guide artifact jobs from the copied snapshot before transmission.

## Conflicts

ostudy merges compatible edits automatically. When both devices changed the same synchronized field, or one deleted a record while the other edited it, ostudy records an explicit conflict instead of silently discarding one side.

A card's schedule is also merged as a whole: when both devices studied or changed the same card, the schedule studied last is kept and the other is recorded as a conflict, so one card never mixes two schedules. Question attempt/evidence rows are treated atomically when concurrent changes cannot be reconciled field-by-field safely. When changes need a decision, **Backup and sync** says how many; choose **Review** to see both versions of each one side by side and choose **Keep this** for the one you want, or **Delete this record**. Managed image or document-file identity mismatches fail rather than selecting arbitrary bytes. Appearance/theme settings never participate because they are excluded from transfer and merge allowlists.

## Local-network requirements

For LAN sync, the devices must be able to reach each other on the same private network. Guest Wi-Fi/client isolation can prevent peer-to-peer connections even when both devices have internet access.

Outgoing LAN offers listen on TCP **49251–49258**. If a receiver reads the QR but cannot reach the sender, verify the displayed private endpoint and host firewall. These port/firewall requirements apply only to local-network sync; direct USB does not use TCP.

## Diagnostics

Native diagnostic events are stored as JSON Lines in a bounded rotating app-private file. Open **Settings → Diagnostics → Current diagnostic state** to see the current log path.

Device synchronization records major phases such as offer creation, transport connection, authentication, package receipt, typed-graph admission, merge start, and merge completion without logging pairing secrets, credential bytes, answer content, or package contents.

On debuggable Android development builds, `adb` can still be used by developers to inspect app-private diagnostics. That development tool is unrelated to product USB Sync and is not required for cable synchronization.

## Invitation lifetime and compatibility

Offers expire after a short period. Create a fresh transfer if one expires or the sending application restarts.

Protocol v3 binds the selected transport into the authenticated pairing transcript. LAN invitations contain a private network endpoint. Direct-USB invitations use the symbolic `usb` endpoint and are delivered only over the accessory stream; pasting a USB invitation into the LAN receive box is rejected.

Both devices need the same library schema to synchronize in both directions. A device refuses a package from a device whose library schema is newer and asks to be updated first. An updated device accepts a package from a device that has not updated yet and brings it up to date before merging; for a device from before the subjects update (schema 6), that converts its folders and collections to subjects. Where the two devices filed a deck or document differently, the merge keeps one subject for it. Update every device, ideally right after a final sync, to avoid that.

The synchronization package has its own versioned format. Granular-scope packages record the selected categories explicitly while retaining package-level compatibility handling for the earlier flashcard/API-settings package format. Received snapshots are migrated and semantically admitted before merge; an older sender cannot bypass current typed-question validation by supplying a structurally valid SQLite file.

## Removing a trusted device

Removing a trusted device forgets its synchronization trust/baseline relationship. It does not delete study data already merged into the local installation. A future transfer with that installation requires new pairing authorization.

## Android QR scanner backend

LAN QR pairing on Android uses local **JourneyApps ZXing Embedded 4.3.0** with **ZXing core 3.5.4**. Decoding runs on the device and does not require Google Play services or a hosted scanner. The app requests camera permission only for the local scan flow, restricts recognition to QR codes, disables barcode-image retention, and bounds decoded content before returning it to Rust. Raw invitation contents are not logged.

Devices without a usable camera, or users who do not grant camera permission, can paste LAN invitations. Direct USB does not use the camera scanner. See [Android development architecture](../development/android.md) and [ADR 0052](../decisions/0052-open-source-android-imaging.md) for the dependency/offline boundary.

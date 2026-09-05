# Flashcards, authoring, and Search

**Flashcards** has two visible tabs: **Decks** and **Search/Filter**. The compact Search label refers to the same workspace. Reusable templates live in **Settings → Card templates**, not another Flashcards tab. Import and export open from contextual deck actions; whole-library backup belongs in Settings → Backup/Sync.

## Navigate between library, deck, and editor

Selecting Decks opens the library root, including when a deck is already open. Selecting the main Flashcards destination also returns to that root. An open deck provides **All decks** to return. Unsaved editor changes still receive the normal discard protection before navigation proceeds.

The app keeps a device-local presentation checkpoint for useful destinations, including an explicitly open deck. It is not a copy of card content, credentials, or the entire screen state. A real practice session can resume over its logical launch destination. If the old deck no longer exists, the return destination safely falls back to the library. This recovery does not resurrect deleted content.

Selecting decks is temporary interaction state. Opening a deck and returning can preserve unchanged active selections during the same process; changed or archived entries are removed. Selection is not written into the restart checkpoint.

## Start from an empty library

When no active or archived decks exist, the library shows one create-deck state. Choose **Create a new deck**, or use the **Import deck from file** icon. There is no empty Unsorted section, empty folder tile, meaningless Select all, or Collapse all.

Once decks exist, **New deck** becomes a toolbar action. Active decks expose optional folder organization. An archived-only library still exposes its archived content and is not presented as a brand-new empty library. See [Organization](flashcards-organization.md) for folders, selection, and keyboard movement.

## Open and manage a deck

An open deck places **All decks**, its title/edit action, description, and actions in a clear vertical flow. Practice is the primary action. Secondary actions adapt to available width: export remains direct where space permits; archive/restore and delete are available in their appropriate action menu. Delete remains a deliberate destructive action rather than an accidental row gesture.

An archived deck is still readable, but practice is unavailable until restored. Archiving and deletion are different: archive retains recoverable material and its history; finalized deletion removes the data and analytics owned by the deleted material. Use Undo promptly when offered after deletion; it is a bounded recovery window, not a permanent trash archive.

Export one deck from its action, or select decks in the library and use the selected export control. Multiple selected decks produce separate portable files. Native system Share is offered for a single deck where supported. Opening the operating system's share sheet does not prove the receiving application completed the transfer. See [Backups and sharing](backups-imports-and-sharing.md).

## Add or edit cards

Inside a deck, **Add flashcard** opens the full rich editor for that destination. **Bulk add** opens multi-row plain-text entry with the deck already fixed. You do not choose that destination again. Search is for finding/managing existing material, not a duplicate global card-creation workflow.

Basic questions and answers support the editor's allowed formatting, lists, links, and managed images. Formatting shortcuts are an authoring convenience; the app stores a validated structured document, not arbitrary executable HTML. Images become managed library assets rather than references to arbitrary local paths or remote image URLs.

A **note** is the shared editable source. A **card** is a presentation you study. A Basic note normally creates one forward presentation. Enabling Reverse creates an independently scheduled reverse presentation of the same source. Editing shared wording or tags therefore affects both directions, while the two cards retain independent review histories and schedules.

A **Fill in the blank** note marks text to hide. One-at-a-time mode selects a blank group for that review; all-together mode hides all groups. It still has one stable scheduled card rather than a separate schedule for every blank. Changing blanks or switching presentation mode does not imply a fresh learning history.

The normal editor reports duplicate candidates and requires deliberate override for an exact duplicate. Bulk entry, template generation, and generated JSON import skip exact duplicates according to their native validation rules. Image-only content is not declared duplicate solely because its searchable text is empty.

## Drafts and saving

Recoverable drafts protect unfinished authoring. They are distinct from saved study content: an autosaved draft does not mean its cards have been committed to the deck. Draft revision checks prevent an older edit from silently overwriting a newer one. Discard and commit are separate operations, and draft-owned staged images follow that lifecycle.

Saving may succeed even when a following list refresh fails. Read an informational reconciliation message before retrying; repeating an already committed action can be different from refreshing the display. The native operation identity and UI guards protect supported retries, but they do not make every newly initiated action interchangeable.

## Reusable templates

Open **Settings → Card templates** to create, edit, preview, or remove reusable structures. Templates contain named fields and safe question/answer layout rules. They do not execute JavaScript, HTML, SQL, or arbitrary expressions.

A template is useful for repeating structures such as term/definition, mechanism/effect, or a comparison. Preview and generation use the same validated rendering path. Generated outputs become ordinary independent notes/cards with fresh schedules; later template edits or deletion do not retroactively change those cards.

The Settings template tab is a full normal workspace. Once opened it can retain its editor state while another Settings tab is selected, and unsaved changes remain protected when leaving the area.

## Search and filters

Open **Search/Filter** for cross-library queries. Common controls stay visible: text, deck/folder scope, sort, and **All filters**. Less frequent availability, schedule, last-studied, tag, and scope criteria live in the filter sheet. Active criteria appear as removable chips so the current scope is visible.

A deck's inline search is simpler and stays within that deck. Clearing global filters should not be confused with changing the underlying cards or schedules. A folder filter resolves current deck membership; it is not a temporary deck.

Results describe actual study presentations: a forward and reverse card can appear separately, while a multi-blank cloze note appears once. Previews use compact question/answer text; open a card to inspect its full rich content rather than treating truncation as missing data.

**Select visible** selects only loaded rows. **Select all matching** resolves the complete current filtered identity set. Bulk move/copy/tag/delete operations account for presentations that share one source note. Review the scope and confirmation before acting.

Use **Practice selected** for exact-card practice. Ending it returns to Search rather than a separate standing Practice tab. Schedule actions such as suspend, bury, reset, and reschedule alter availability/current schedules; they do not invent review events. [Practice and analytics](practice-and-analytics.md) explains those differences.

Current tags shown on recent reviews in Analytics can launch Search by stable tag identity. Historical tag text remains readable but is not a link to a current tag. If the target is no longer available, Search asks you to clear that unavailable filter explicitly instead of silently showing the whole library.

## Importing content

The library's **Import deck from file** action accepts portable `.studydeck` and compatible generated `.ostudy.json` input. There is no generated-import approval/preview queue. JSON is validated as a whole before the transaction; invalid content leaves the library unchanged and produces a useful error path. Imported study content receives local schedules rather than another learner's history.

A new imported deck appears in Unsorted decks until organized locally. Folder membership and the sender's personal learning history are not part of the portable deck. Whole-library recovery requires `.ostudybackup`, not an individual deck export.

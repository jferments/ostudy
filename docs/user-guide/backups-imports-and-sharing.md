# Backups, imports, and sharing

A deck shared with a classmate, generated card input, a personal disaster-recovery backup, and trusted-device synchronization have different purposes. Choosing the right operation avoids accidentally transferring personal history or overwriting a library.

## Choose the right format

| Format or operation | Use it for | Personal review history | Saved AI credentials |
| --- | --- | --- | --- |
| `.studydeck` | One portable deck | Excluded | Excluded |
| Compatible Anki `.apkg` | Convert existing Anki card presentations | Not imported | Not imported |
| Quizlet Export text | Bring a Quizlet set's terms and definitions | Not imported | Not imported |
| `.ostudy.json` | Strict generated-card authoring input | Excluded | Excluded |
| `.ostudybackup` | Whole-library recovery | Included | Excluded |
| Trusted-device sync offer | Explicit one-way exchange of the selected supported study-data graph | Included within selected scope | Separate sender opt-in and secure-store requirements |

A backup is not automatically encrypted just because credentials are excluded. It contains private study material and history; protect its storage and any channel used to transfer it.

## Import a deck or generated cards

Open **Library → Flashcards** and choose **Import deck** beside the Library tabs (on a phone, beside **New deck** under the decks). **Choose a file** accepts `.studydeck`, compatible Anki `.apkg`, and compatible `.ostudy.json` files, **Paste from Quizlet** takes a Quizlet set's Export text, and **Into subject** files the new deck in a subject, starting on the one you have open. If the deck cannot be filed, it is still imported, under No subject. There is no standing Import/Export tab and no separate generated-import preview/approval queue.

A new imported deck starts in **No subject** until you move it with **Move to subject…**. The receiving library owns subject membership and schedules. Generated `.ostudy.json` and `.studydeck` imports open the imported deck when they finish.

### Portable `.studydeck`

Current `.studydeck` version 3 is a versioned SQLite interchange package containing one deck's notes, active presentations, tags, supported rich content, and managed study assets. Basic, Fill in the blank, single-best-answer, select-all, true/false, matching, grouping, ordering, and manually authored image-occlusion questions retain their native typed definitions. Image-occlusion source images and other managed assets referenced by the question travel with the deck.

The package deliberately excludes the exporting learner's schedules/reviews, captured practice attempts, remediation/history, personal analytics, application settings, provider profiles/credentials, and unrelated reusable authoring templates. The importer validates the complete typed graph before mutation, remaps managed asset identities when needed, rebuilds stable interactive presentation ownership, and assigns fresh local schedules. The receiver therefore studies shared content using their own history, not the sender's due dates.

Older supported `.studydeck` versions remain readable for the card kinds they could represent. A typed interactive graph cannot masquerade as an older package version and be flattened into another kind; incompatible structure is rejected before library mutation.

### Anki `.apkg`

Choose an Anki deck package through the same import dialog. Older-compatible packages can be converted directly without installing Anki libraries in oStudy. When a newer package is rejected, open it in Anki and export an **Anki Deck Package (.apkg)** with **Support older Anki versions** and **Include media** enabled. Renaming a file does not convert its format. Compressed modern collections, collection backups, and arbitrary standalone Anki databases are not supported by this first adapter.

The importer follows the cards actually stored in the package. Common Basic, reverse, and optional-reverse presentations become independent editable Basic cards with fresh schedules. Compatible simple cloze presentations become native **Fill in the blank** cards. Complex cloze cards that contain extra answer fields or media keep their rendered presentation as Basic cards rather than losing those fields. Anki linked-note editing and review history do not carry over. Subdecks are combined into one imported deck, with source deck paths retained as tags rather than recreated as subjects.

For ordinary Basic cards, Anki's common back-template repetition of the front is removed from the stored answer. oStudy's reveal shows the question once, above the answer, rather than repeating it inside the answer. Turn off **Settings → Review → Display both question and answer when card is flipped** if you prefer the answer alone. Fill in the blank imports retain the blank/context on their revealed side automatically.

Anki `type:`/typed-answer cards preserve the question and answer as ordinary reveal cards. The interactive typing/comparison widget itself is not retained. Supported local images and readable text formatting are converted to native content. Arbitrary CSS is normalized to oStudy's native appearance and reported as an aggregate appearance conversion instead of producing one warning per card.

Anki templates may also contain JavaScript. oStudy **never executes imported JavaScript**. Script elements and inline event-handler attributes are removed before native conversion. If the remaining front and back still contain complete static study content, the card is retained and counted as **JavaScript recovered as static content**. If removing the code leaves an empty or incomplete question/answer, the card is skipped because its learning meaning may have depended on dynamic generation. Other unsupported active behavior, add-ons, audio/video, TTS, image occlusion, nested cloze, genuinely unsupported template filters, or missing required images can likewise cause an affected card to be skipped rather than replaced with a misleading placeholder.

While a large Anki deck is importing, the dialog shows the current phase and card-level progress. After the package has been read and its card total is known, the progress view shows **cards processed / total**, plus the number imported and skipped so far. The phases include reading the package, converting cards, importing media, and saving the deck. Progress display is advisory; losing a UI progress listener does not change whether the transactional import succeeds.

After the import commits, the dialog stays open as **Anki import complete**, with an import report and **Open imported deck**. It shows imported/skipped/content-simplified totals plus aggregate counts for native appearance conversion, JavaScript static recovery, typed-answer conversion, native Fill in the blank conversion, and complex-cloze fallback. Expand compatibility details for actual per-card problems. If no compatible cards are found, no deck is created. Nothing executable in the package is run, and no network content is fetched.

The compatibility report is a post-import result, not an approval preview. Do not import the same file again merely because some cards were skipped. See the [Anki adapter reference](../reference/anki-import.md) for exact format, semantic conversion, extension boundaries, resource limits, and JavaScript-salvage rules.

### Quizlet sets

Quizlet has no deck file to download; it exports a set as text. In Quizlet, open the set, choose **⋯** then **Export**, and copy the text. In oStudy's **Import deck** dialog choose **Paste from Quizlet**, paste the text, and name the new deck. Quizlet's defaults (a tab between term and definition, a new line between cards) are recognized, and **Between term and definition** (Tab or Comma) and **Between cards** (New line or Semicolon) match the other Export choices. A definition that spans several lines needs **Semicolon** between cards in Quizlet's Export. The dialog shows how many cards it found and the first one before anything is saved.

Each term and definition becomes a Basic card with a fresh schedule, in a new deck filed by **Into subject**. Images, audio, starred terms and Quizlet's study progress do not carry over, and exact duplicates are skipped. Cards are saved 500 at a time; if a later batch fails, the message says how many were added, and **Import** again adds only the missing ones to the same deck.

### Generated `.ostudy.json`

This is strict UTF-8 data-only input. It creates one named deck or appends to exactly one matching active deck according to the format. Basic/Reverse and Fill in the blank content use restricted Markdown and the supported cloze grammar.

The entire file is validated before one transaction. An invalid card leaves the library unchanged and produces a path-specific error; exact duplicates are skipped automatically. The file cannot supply arbitrary internal IDs, SQL, HTML, local paths, images, schedules, reviews, settings, analytics, or deletion/overwrite instructions.

See the [format reference](../reference/generated-json-import.md), [generator instructions](../reference/flashcard-import-agent.md), [JSON Schema](../ostudy-flashcards-v1.schema.json), and [example](../examples/cardiovascular.ostudy.json). Document ingestion of CSV as source material is a separate feature and does not imply a CSV-to-flashcard interchange importer. Other interchange formats are supported only where explicitly documented.

## Export or share decks

In the Workbench, open a deck's **…** menu and choose **Export…**, then **Save file** to save a `.studydeck` through the platform's save dialog. Where the platform supports system sharing, **Share** hands the file to the operating system; that does not prove a recipient received it. Export is one deck at a time and is never a whole-library archive.

A successful import remains committed even if a subsequent screen refresh fails. Refresh the view rather than immediately assuming the write failed and initiating a new import.

## Create a whole-library backup

Open **Settings → Backup and sync** and choose **Back up library** (or **Share backup** where the platform supports sharing). A `.ostudybackup` captures a consistent snapshot of the library database and the managed files needed to restore it: subjects, decks, notes and cards with their question definitions, schedules, reviews and practice attempts, study activity, tags and templates, ordinary settings, managed images, source documents and everything extracted from them, learning plans, study guides with your notes and edits, PDF annotations, lessons, Tutor conversations and projects, and generation evidence.

Supported backups also preserve recoverable card, template, bulk-entry, and nonsecret provider-metadata drafts with their staged images. Nonsecret provider profiles, task defaults, and appropriate redacted metadata belong to the library; saved credential bytes do not. Device-local presentation checkpoints and browser-local tutorial progress are not a full portable UI session.

Backup creation is blocked while document work or transient file/asset operations could make the database and managed files inconsistent. Successfully persisted editor drafts do not by themselves prevent backup. Finish or deliberately cancel active work and follow the displayed message rather than copying a live database file manually.

Create backups after substantial authoring, before migration-heavy testing, before replacing a device, and before risky cleanup. Keep multiple generations and at least one copy outside the app data directory and outside a single potentially failing disk.

## Restore deliberately

Restoration replaces the personal library; it does not merge like importing one deck. Save any work you need and make a separate backup of the current library before replacing it.

Choose **Restore backup**, select the `.ostudybackup`, and confirm **Restore library**. Finish or cancel active document, Tutor, preparation, or device-sync work first. oStudy validates the package, stages an isolated replacement, temporarily blocks library operations, activates the replacement, and refreshes the interface. The native application stays running: **normal restoration does not require closing and reopening oStudy**. Android shared backups require the same explicit whole-library replacement confirmation.

Completion reports decks, cards, and documents only after the restored database reopens and reconciliation succeeds. A backup containing documents but no decks is not empty. A valid empty destination, a file-picker completion, or an old “restore prepared” message does not prove activation. Verify documents/guides, managed images, schedules, recent reviews, questions/attempts, settings, and recoverable drafts. Do not regenerate completed document material merely to test restoration.

Saved API keys remain in the operating-system secure store and are not part of the backup. Restoration does not delete that store. All session-only credentials held by the running native process are cleared during live library replacement, including orphaned entries; enter any needed session keys again. Two desktop production instances with different XDG roots still use the production keyring namespace; development has a separate namespace.

Keep the original backup after restoration. oStudy also retains the previous library and selected package under its `restore-transactions` recovery area. These copies consume disk space and are not a substitute for an external backup. Do not remove recovery files, SQLite journal files, or `library-owner.lock` to bypass a failure.

An ordinary validation failure leaves the active library intact. If activation fails, oStudy attempts to reopen the previous coherent library. An interrupted process is recovered from its durable journal on the next startup. If recovery cannot be verified, commands remain blocked and the recovery files stay available; preserve them and inspect diagnostics rather than resetting or reinstalling. A second updated process cannot open the same library while its owner is running. Older binaries that predate the ownership lock must also be closed before any restore test.

Do not rename an arbitrary SQLite file to masquerade as a backup. The package version, schema, inventory, paths, and managed bytes are validated together. Installing an older binary is not a guaranteed rollback for a newer data schema; preserve a compatible backup before development testing.

## Sync is not backup

[Device synchronization](device-sync.md) sends the selected supported study-data graph over an authenticated encrypted local-network or direct-USB transport and merges it on the receiver. It is not automatic cloud backup, continuous background replication, or permission for the receiver to upload its own library during acceptance.

When **Flashcards** is selected, question definitions, their presentations, practice attempts and review links, and question images travel and are checked like the rest of the flashcard data; records whose fields must agree are merged whole rather than field by field. A malformed package is rejected before anything is installed or merged. Editor drafts, question generation evidence and other local records stay on the sending device.

Documents, Study Guides, Notes and Lessons have explicit selectable scopes; Study Guides require Documents. Links between documents and flashcards travel only when both are selected. Ordinary appearance and application settings stay on each device, and so do Tutor conversations and the knowledge extracted from documents. Provider settings and saved keys need the sender's explicit choice for each offer. For disaster recovery of the entire personal library, use `.ostudybackup` rather than assuming a selected sync scope is a complete backup.

## Safety and factual accuracy

Imported content is untrusted even when supplied by a friend or a local model. Native validation protects data and execution boundaries; it cannot prove that a learning claim is correct. Review generated/shared material against its source.

For destructive or restore tests, use isolated temporary data. A separate source worktree does not automatically mean a separate app library. The [local testing guide](../development/local-testing-and-handoff.md) provides isolation and install cautions.

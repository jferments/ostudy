# Backups, imports, and sharing

A deck shared with a classmate, generated card input, a personal disaster-recovery backup, and trusted-device synchronization have different purposes. Choosing the right operation avoids accidentally transferring personal history or overwriting a library.

## Choose the right format

| Format or operation | Use it for | Personal review history | Saved AI credentials |
| --- | --- | --- | --- |
| `.studydeck` | One portable deck | Excluded | Excluded |
| Compatible Anki `.apkg` | Convert existing Anki card presentations | Not imported | Not imported |
| `.ostudy.json` | Strict generated-card authoring input | Excluded | Excluded |
| `.ostudybackup` | Whole-library recovery | Included | Excluded |
| Trusted-device sync offer | Explicit one-way exchange of the supported study-data graph | Included within supported scope | Separate sender opt-in and secure-store requirements |

A backup is not automatically encrypted just because credentials are excluded. It contains private study material and history; protect its storage and any channel used to transfer it.

## Import a deck or generated cards

Open **Flashcards → Decks** and use the **Import deck from file** icon. It accepts `.studydeck`, compatible Anki `.apkg`, and compatible `.ostudy.json` files. There is no standing Import/Export tab and no separate generated-import preview/approval queue.

A new imported deck starts in **Unsorted decks** until you organize it locally. The receiving library owns folder membership and schedules.

### Portable `.studydeck`

This versioned SQLite interchange package contains one deck's notes, active presentations, tags, supported rich content, and managed study images. It excludes the exporting learner's schedules/reviews, personal practice and analytics, application settings, provider profiles/credentials, and unrelated reusable authoring templates.

The importer validates the package graph and assigns fresh local schedules. The receiver studies the shared content using their own history, not the sender's due dates. A portable deck is therefore useful for sharing content but is not sufficient to recover a personal library.

### Anki `.apkg`

Choose an Anki deck package through the same import dialog. Older-compatible packages can be converted directly without installing Anki libraries in oStudy. When a newer package is rejected, open it in Anki and export an **Anki Deck Package (.apkg)** with **Support older Anki versions** and **Include media** enabled. Renaming a file does not convert its format. Compressed modern collections, collection backups, and arbitrary standalone Anki databases are not supported by this first adapter.

The importer follows the cards actually stored in the package. Common Basic, reverse, and optional-reverse presentations become independent editable Basic cards with fresh schedules. Compatible simple cloze presentations become native **Fill in the blank** cards. Complex cloze cards that contain extra answer fields or media keep their rendered presentation as Basic cards rather than losing those fields. Anki linked-note editing and review history do not carry over. Subdecks are combined into one imported deck, with source deck paths retained as tags rather than recreated as folders.

For ordinary Basic cards, Anki's common back-template repetition of the front is removed from the stored answer. The default oStudy reveal therefore shows the answer rather than repeating the question and answer together. Enable **Settings → Review/Scheduling → Display both question and answer when card is flipped** if you prefer the front to stay visible after reveal. Fill in the blank imports retain the blank/context on their revealed side automatically.

Anki `type:`/typed-answer cards preserve the question and answer as ordinary reveal cards. The interactive typing/comparison widget itself is not retained. Supported local images and readable text formatting are converted to native content. Arbitrary CSS is normalized to oStudy's native appearance and reported as an aggregate appearance conversion instead of producing one warning per card. JavaScript, add-ons, audio/video, TTS, image occlusion, nested cloze, and genuinely unsupported template filters are not reproduced yet. A card that depends on unsupported behavior or has a missing required image is skipped rather than replaced with a misleading placeholder.

After the import commits, the dialog stays open with an **Anki import report**. It shows imported/skipped/content-simplified totals plus aggregate counts for native appearance conversion, typed-answer conversion, native Fill in the blank conversion, and complex-cloze fallback. Expand compatibility details for actual per-card problems. If no compatible cards are found, no deck is created. Nothing in the package is executed or fetched from the network.

The compatibility report is a post-import result, not an approval preview. Do not import the same file again merely because some cards were skipped. See the [Anki adapter reference](../reference/anki-import.md) for exact format, semantic conversion, extension boundaries, and resource limits.

### Generated `.ostudy.json`

This is strict UTF-8 data-only input. It creates one named deck or appends to exactly one matching active deck according to the format. Basic/Reverse and Fill in the blank content use restricted Markdown and the supported cloze grammar.

The entire file is validated before one transaction. An invalid card leaves the library unchanged and produces a path-specific error; exact duplicates are skipped automatically. The file cannot supply arbitrary internal IDs, SQL, HTML, local paths, images, schedules, reviews, settings, analytics, or deletion/overwrite instructions.

See the [format reference](../reference/generated-json-import.md), [generator instructions](../reference/flashcard-import-agent.md), [JSON Schema](../ostudy-flashcards-v1.schema.json), and [example](../examples/cardiovascular.ostudy.json). Document ingestion of CSV as source material is a separate feature and does not imply a CSV-to-flashcard interchange importer. Other interchange formats are supported only where explicitly documented.

## Export or share decks

Use a deck's contextual **Export** action or select decks and use the selected-deck export control. Wide layouts may show a direct control; compact layouts place secondary actions in the relevant action menu. Multiple selected decks produce separate files, not a whole-library archive.

The export dialog uses the native save flow. A single-deck native **Share** path is available where the platform supports system sharing. Opening a share sheet hands the file to the operating system; it does not prove a recipient received it. Canceling a later file in a multi-deck export does not undo files already saved, so read the partial-result message.

A successful import remains committed even if a subsequent screen refresh fails. Refresh the view rather than immediately assuming the write failed and initiating a new import.

## Create a whole-library backup

Open **Settings → Backup/Sync** and use the library backup controls. A `.ostudybackup` captures a coherent SQLite snapshot and the managed files required to restore it, including decks/folders, notes/cards, tags/templates, schedules, reviews, sessions, analytics source records, ordinary settings, managed images, retained source documents, canonical document derivatives, learning plans/topics, study-guide revisions and learner overlays, and their persisted relationships.

Supported backups also preserve recoverable card, template, bulk-entry, and nonsecret provider-metadata drafts with their staged images. Nonsecret provider profiles, task defaults, and appropriate redacted metadata belong to the library; saved credential bytes do not. Device-local presentation checkpoints and browser-local tutorial progress are not a full portable UI session.

Backup creation is blocked while document work or transient file/asset operations could make the database and managed files inconsistent. Successfully persisted editor drafts do not by themselves prevent backup. Finish or deliberately cancel active work and follow the displayed message rather than copying a live database file manually.

Create backups after substantial authoring, before migration-heavy testing, before replacing a device, and before risky cleanup. Keep multiple generations and at least one copy outside the app data directory and outside a single potentially failing disk.

## Restore deliberately

Restoration replaces the personal library; it does not merge like importing one deck. The application validates and stages a replacement, then applies it through startup so a partially copied database or managed directory is not exposed as a normal library.

Restart when instructed. Confirm decks, images, schedules, recent reviews, documents/guides, notes, settings, and unfinished drafts that should have been backed up. Reconfigure or verify provider credentials separately. Test a small session and preserve the original backup until recovery is confirmed.

Do not rename an arbitrary SQLite file to masquerade as a backup. The package version, schema, inventory, paths, and managed bytes are validated together. Installing an older binary is not a guaranteed rollback for a newer data schema; preserve a compatible backup before development testing.

## Sync is not backup

[Device synchronization](device-sync.md) sends a supported study-data snapshot over an authenticated encrypted private-LAN session and merges it on the receiver. It is not automatic cloud backup, continuous background replication, or permission for the receiver to upload its own library during acceptance.

Ordinary appearance/application settings stay device-local. Provider configuration and available saved credentials require a new explicit sender opt-in per offer. The document/study-guide learning-artifact graph is not currently a promise of full cross-device synchronization: use a complete-library backup/restore for that recovery scope rather than assuming every table or source file is synchronized.

## Safety and factual accuracy

Imported content is untrusted even when supplied by a friend or a local model. Native validation protects data and execution boundaries; it cannot prove that a learning claim is correct. Review generated/shared material against its source.

For destructive or restore tests, use isolated temporary data. A separate source worktree does not automatically mean a separate app library. The [local testing guide](../development/local-testing-and-handoff.md) provides isolation and install cautions.

# Library updates and recovery

Installing a newer oStudy application and upgrading its library are separate operations. The operating system installs the application. On startup, oStudy checks the selected library and automatically upgrades a supported older schema when necessary. There is no automatic application downloader or configured apt update repository in this workflow.

## Keep school work separate from development

Use the installed production application for the school library and the maintained source-build launcher for development. The normal Linux directories are:

| Profile | Application identity | Default library directory |
| --- | --- | --- |
| Production | `io.github.jferments.ostudy` | `~/.local/share/io.github.jferments.ostudy/` |
| Development | `io.github.jferments.ostudy.dev` | `~/.local/share/io.github.jferments.ostudy.dev/` |

An explicit `XDG_DATA_HOME` changes the parent directory. The development runtime refuses the production identity, and production and development keep separate credential namespaces. Separate source worktrees still share the ordinary development library unless their XDG roots are isolated.

There is no automatic synchronization or merging between these libraries. A deck import transfers content with local schedules; a whole-library restore replaces the destination. Neither is a safe way to combine two independently evolving review histories. See [Backups, imports, and sharing](backups-imports-and-sharing.md).

## Install an application update

Keep a portable `.ostudybackup` outside the application directory, and retain the previous qualified application package with its checksums and build information. Close oStudy before replacing the installed package. Install the selected qualified `.deb` through the package manager, then launch the installed application normally. The package owns program and integration files, not the school library. See [Linux installation](../installation/linux.md).

An application version and a library schema are different identifiers. Installing a new build with the same supported schema does not require a schema migration or create another automatic migration checkpoint. The current library schema is `ostudy-release-v1` schema 19: schema 3 adds deck-merge receipts, schema 4 adds saved generation guidance, schema 5 records what the scheduler predicted at each new review and how long the card was on screen (reviews from before that update keep no such record), schema 6 turns deck folders and document collections into subjects, schema 7 keeps your study days when cards are deleted, schema 8 stores what you draw and type on your documents, schema 9 stores lessons, schema 10 remembers which marks your cards were made from, schema 11 keeps the spoken narration of lessons, schema 12 remembers how far you have read each study guide, schema 13 lets a lesson be saved as a slide deck, schema 14 lets a PDF be saved with your annotations drawn in, schema 15 keeps a lesson's teacher's lecture, schema 16 keeps your saved voices and the voice each lesson is read in, schema 17 keeps your podcasts, schema 18 stores your notebooks, and schema 19 stores what you write on lecture slides and study guide pages, the last fourteen described below. Libraries at schemas 1–18 upgrade forward through the checkpointed path below.

Builds may still share the application version `0.1.0`. Do not identify a candidate or rollback package by that number alone: retain `BUILD-INFO.txt`, the exact source SHA and package checksum. A historical library without an earlier build receipt has an unknown originating build; the new application does not guess it.

## The subjects update (schema 6)

A subject is one course: it holds the course's documents, guides and decks. Deck folders and document collections both become subjects; Library lists the subjects once, beside tabs for each subject's lessons, guides, documents and flashcards. Each deck and each document is in at most one subject.

- Every folder becomes a subject with its decks. A folder and a collection with the same name, ignoring capitals and surrounding spaces, become one subject, but only when no other folder or collection uses that name. Otherwise both are kept under their own names.
- A document that was in several collections stays in the one it joined first and leaves the others, keeping the page ranges set there. A document in no collection joins the folder of its generated deck.
- A deck generated from a document, or a collection's study bundle deck, joins that document's or collection's subject. Other decks stay in their folders.
- Collection flashcards were copies of the original cards, each with its own schedule. Unedited copies fold back into their originals: the original keeps the schedule of whichever card you studied, reset or rescheduled most recently, and reviews of the copy count under the original in Progress, history and Search/Filter. No review is moved, changed or deleted. Copies you edited, and copies whose original is gone, stay as cards of their own.
- A study bundle deck left without cards is archived. Studying a subject's materials now studies the decks filed in it, live.

Older builds cannot open a schema-6 library. Restoring the pre-upgrade checkpoint returns the schema-5 library as it was. A `.ostudybackup` made before the update still restores, and the restored library gets the same subjects as updating in place.

If you synchronize several devices, update all of them, ideally right after a final sync. A device without the update refuses a package from an updated device and asks to be updated first; an updated device still accepts a package from one that has not updated yet. See [Device sync](device-sync.md).

## The study activity update (schema 7)

Study days, streaks and study time now belong to you rather than to your cards. When deleted cards or decks are cleaned up after the undo window, oStudy keeps how many reviews you made in each hour of each day, how many you remembered, how many were new cards and how long they took. The activity calendar, streaks and day totals count those kept totals with your stored reviews, so deleting a deck no longer shortens a streak. Progress views of your current cards (remembered, subjects, the reviews chart) still count only cards that exist.

Totals are kept by hour, so changing "New day starts at" in Settings regroups past days the same way it regroups stored reviews. The update itself writes no rows: totals are kept from the first deletion after it, and the Progress page builds its saved per-deck summaries the first time it opens.

Older builds cannot open a schema-7 library. Restoring the pre-upgrade checkpoint returns the schema-6 library as it was. Kept totals synchronize between updated devices with flashcards.

## The document annotations update (schema 8)

Schema 8 adds a place for the ink, highlights, shapes and text notes you add to a document's pages in the annotator. The update itself writes nothing: the table starts empty, and your documents and their files are unchanged.

Older builds cannot open a schema-8 library. Restoring the pre-upgrade checkpoint returns the schema-7 library as it was. Annotations are part of whole-library backups but stay on the device that made them: device sync does not carry them yet.

## The lessons update (schema 9)

Schema 9 adds a place for a subject's lessons: short teaching sequences written from the subject's documents, each kept with every earlier version of its text, and where you stopped in each one. Finishing a lesson is not a review: it never changes a card's schedule. The update itself writes nothing: the tables start empty. This release has no screen for writing or reading lessons yet, so the tables stay empty in ordinary use.

Older builds cannot open a schema-9 library. Restoring the pre-upgrade checkpoint returns the schema-8 library as it was. Lessons are part of whole-library backups but stay on the device that made them: device sync does not carry them yet, because it does not carry the extracted knowledge they are written from.

## The cards-from-marks update (schema 10)

Schema 10 remembers which highlight or note on a document each card you make from it came from, with the page and a copy of the marked words, so you can study the cards from what you marked. Deleting the mark or the document keeps the card. The update itself writes nothing: the table starts empty.

Older builds cannot open a schema-10 library. Restoring the pre-upgrade checkpoint returns the schema-9 library as it was. These links are part of whole-library backups but stay on the device that made them, like the annotations themselves; shared decks do not carry them.
## The lesson narration update (schema 11)

Schema 11 keeps the audio of lesson narration you have listened to, so a step is spoken by the voice service once and then plays from your library, and it records which voice lessons use. The update itself writes nothing: the tables start empty.

Older builds cannot open a schema-11 library. Restoring the pre-upgrade checkpoint returns the schema-10 library as it was. Narration audio is part of whole-library backups but stays on the device that made it, like the lessons it belongs to. Deleting a lesson keeps its audio for now, because the same sentences can be spoken again in another lesson.

## The guide reading update (schema 12)

Schema 12 remembers which sections of a document's study guide you marked as read, and the section you last opened, so the guide can show how far you are and reopen where you left off. Remaking a guide keeps a section's mark while its text is unchanged; a rewritten section reads as unread again. Deleting the guide or the document clears what was read. Reading a guide is not a review: it never changes when cards are due. The update itself writes nothing: the tables start empty.

Older builds cannot open a schema-12 library. Restoring the pre-upgrade checkpoint returns the schema-11 library as it was. Reading progress is part of whole-library backups but stays on the device that made it: device sync does not carry it.

## The lecture export update (schema 13)

Schema 13 lets a lesson's lecture be saved as a PowerPoint deck. It rebuilds the small table of exports waiting to be saved so it accepts a slide deck, keeping every entry. The update adds no rows.

Older builds cannot open a schema-13 library. Restoring the pre-upgrade checkpoint returns the schema-12 library as it was.

## The annotated PDF update (schema 14)

Schema 14 lets a PDF be saved with your annotations drawn into its pages. It rebuilds the small table of exports waiting to be saved so it accepts an annotated PDF, keeping every entry. The update adds no rows.

Older builds cannot open a schema-14 library. Restoring the pre-upgrade checkpoint returns the schema-13 library as it was.

## The teacher's lecture update (schema 15)

Schema 15 keeps the teacher's lecture written for a lesson: what the teacher says over each slide and the questions they ask the class, with the teacher's reply to each answer. A lecture belongs to one saved version of its lesson and goes when the lesson does. Answering the teacher's questions is practice: it is not saved and never changes when cards are due. The update itself writes nothing: the table starts empty.

Older builds cannot open a schema-15 library. Restoring the pre-upgrade checkpoint returns the schema-14 library as it was. Lectures are part of whole-library backups; device sync leaves them out, like the lessons they belong to.

## The saved voices update (schema 16)

Schema 16 keeps the voices you save for reading lessons aloud, each a short sample of the voice with the words it says, and the voice each lesson was first read in, so a lesson keeps its voice when you choose another. A lesson's record goes when the lesson does. The update itself writes nothing: both tables start empty. Drama 3 reads in **Relaxed man**, one of the two voices that come with oStudy, until you choose another. A lesson read aloud before the update takes the voice chosen when it is next read. Drama 3 audio made before the update had no single voice, so it is read again once, in that voice; Kokoro audio is reused.

Older builds cannot open a schema-16 library. Restoring the pre-upgrade checkpoint returns the schema-15 library as it was. Saved voices are part of whole-library backups; device sync leaves them out, like lesson narration.

## The podcasts update (schema 17)

Schema 17 keeps the podcasts you make from a subject's lessons: the conversation the two hosts have, which lessons it covers, and the voice each host speaks in. A podcast keeps playing after its lessons are rewritten or deleted, and goes when its subject does. Its audio is kept the same way as lesson narration. The update itself writes nothing: the table starts empty.

Older builds cannot open a schema-17 library. Restoring the pre-upgrade checkpoint returns the schema-16 library as it was. Podcasts are part of whole-library backups; device sync leaves them out, like the lessons they are made from.

## The notebooks update (schema 18)

Schema 18 stores your notebooks: their pages, and what you draw, type and link on them in Library's Notes page. It adds three empty tables and changes no existing rows. Whole-library backups keep notebooks; device sync leaves them on the device that made them.

Older builds cannot open a schema-18 library. Restoring the pre-upgrade checkpoint returns the schema-17 library as it was, without any notebooks made since.

## The slide and page ink update (schema 19)

Schema 19 stores what you write and draw on a lesson's lecture slides and on a study guide's designed pages. It adds one empty table and changes no existing rows. Your marks go when their lesson or document does. Whole-library backups keep them; device sync leaves them on the device that made them.

Older builds cannot open a schema-19 library. Restoring the pre-upgrade checkpoint returns the schema-18 library as it was, without any marks made since.

## What happens when migration is required

Before ordinary library commands become available, oStudy takes exclusive cooperating-process ownership and inspects schema identity, compatibility and recovery state. A supported older library is copied into a private recovery transaction. The SQLite snapshot includes committed WAL data; managed assets, documents, transfers and cache trees are copied and checked too. Saved operating-system credentials are not read or changed.

The complete recovery checkpoint must verify before any migration is attempted. All required migration work happens on a separate candidate. That candidate must reach the expected schema and pass native admission, integrity, relationship and applicable preservation checks before activation. The original active library is not progressively migrated.

The startup screen reports checkpoint creation, private-copy migration, verification and activation, with a bar showing how much of the whole update is done. The bar counts the bytes the update copies and checks against an estimate made from the library's size, so it can pause just short of the end while the activated library is opened and checked. A late-loading window can recover the current progress message and bar. These messages do not grant database access; normal startup readiness remains independent. After successful activation, reconciliation and commit, a dismissible notice says that the library was updated and its recovery checkpoint retained.

The shared activation journal coordinates the database and its journals with managed files. A failed operation before commit attempts to restore the previous coherent state. If the process is interrupted, the next launch reads the journal before opening the library. A committed operation is not reversed merely because bookkeeping cleanup failed.

## When an update fails

Do not reset the library, delete SQLite sidecars, remove `library-owner.lock`, or repeatedly reinstall packages. Preserve the app data and recovery files. Other programs and historical binaries that ignore oStudy's ownership lock must also be closed.

A failed or interrupted preparation does not silently retry at every launch. The **Library update stopped** dialog explains that saved copies were kept and that closing leaves them as they are. When checkpoint metadata is available, **Recovery options** offers **Restore backup…** or **Try update again…**. Both lead to an explanation and a separate confirmation before any recovery action. The checkpoint is verified again before restoration; its mere presence is not proof that it is usable.

Restoration requires a second destructive confirmation. It reinstates the old library bytes instead of executing reverse SQL. Any work newer than the checkpoint disappears from the active library, but the replaced library is retained separately in the recovery transaction. oStudy then closes. The restoration does **not** install, remove or downgrade an application package.

### After restoring: keep the old library or try the update again

**Your library is restored** means restoration succeeded. It is not another failure. The newer installed oStudy version still needs to update the older library format before it can open your study workspace.

**Keep restored library** closes oStudy without changing that library. Install the retained previous compatible oStudy package separately to use the library as it is.

**Update on next launch** allows the library update to run again when you reopen oStudy. It is not a retry of the restoration, and clicking it does not update anything immediately. This is optional; first address the reason the update failed, such as low disk space or a software defect. Choosing retry does not fix that cause, so an unchanged problem can fail again. The **Library update scheduled** message distinguishes permission to retry from an update actually completing. Existing saved copies remain.

To return to an older release, finish library recovery first, close oStudy and install the retained compatible previous package. The newer executable will not immediately re-upgrade an explicitly restored older library without a new decision. An executable whose target schema matches that restored checkpoint can admit it after checking the recovery state. Never install an older binary while an activation journal is unresolved: that binary might not understand the newer journal format.

A future, forged or unsupported schema is refused rather than relabelled. Pre-public libraries still require the existing explicit recovery procedure; this feature does not restore automatic pre-public conversion. If native recovery cannot establish a coherent state, it preserves the files and keeps normal use blocked. Keep the displayed operation context and inspect local diagnostics rather than deleting evidence.

## Retained copies and disk space

Recovery transactions live under `restore-transactions/<operation-id>/` within the selected profile. They contain checkpoint metadata and a verified checkpoint, the migration candidate while it exists, and previous or failed-library entries according to the reached phase. Metadata records source/target lineage and schema, application identity, timestamp, checkpoint hash and available build identities.

These copies are retained, not automatically pruned. Allow space for the checkpoint and candidate in addition to the active library; repeated failures or restoration can retain further copies. If copying, verification or migration runs out of space or encounters an IO error, the operation stops rather than bypassing the checkpoint requirement. Do not free space by deleting unresolved transaction files.

An internal checkpoint on the same disk is not an external disaster-recovery backup, is not automatically encrypted, and is not a portable `.ostudybackup` package. Do not rename its database to make it look like one. Continue making ordinary backups after substantial school work and keep at least one copy outside the application directory and the same physical disk.

The recovery dialog handles an outstanding failed/interrupted update. This release does not add a general Settings browser for all historical checkpoints, automatic checkpoint retention controls, an application rollback installer, or rollback protection for every same-schema application change.

For disposable testing, use the [upgrade acceptance procedure](../development/library-upgrade-acceptance.md). Do not use the school library as an upgrade failure fixture.

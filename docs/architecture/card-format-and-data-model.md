# Card format and data model

ostudy separates editable source, study presentations, current scheduling, and historical evidence. [The comprehensive architecture](overview.md) explains the surrounding services; this document defines the distinctions most likely to be broken by an authoring, search, or import change.

## Notes are not scheduled presentations

A **note** owns editable content, tags, type/template-origin information where applicable, and source/topic associations. A **card** owns a stable study presentation. One Basic note can have forward and reverse cards with separate schedules/reviews. A cloze note has one scheduled card, even when it contains several blank groups.

Frontend `CardNote` carries editable source. `CardPresentation` carries a rendered scheduled/search identity and relevant asset metadata. A single generic card DTO would confuse edits to shared source with changes to one presentation's learning history.

Decks contain notes/cards. Optional folders organize decks; each deck has at most one folder. Folder membership/order does not define another scheduler. Deleting a folder returns its decks to Unsorted; archiving and deleting a deck have different lifecycle semantics.

## Two rich-content versions with different purposes

The persisted **`tiptap-json` content transport envelope remains version 1**. The canonical rich document root independently supports **`attrs.schemaVersion = 2`**, while supported version-1 documents remain readable. These numbers must not be conflated or used to justify a destructive schema migration.

Version 2 adds subscript/superscript marks, tables/cells/rows/headers with validated spans/widths, and ordinary/answer-key task lists with stable item identity. Existing supported paragraphs, headings, lists, code, blockquotes, links, styles, and managed images remain constrained by native validation.

Rust treats the JSON as untrusted. It validates root/type/version, allowed nodes and marks, parent-child structure, attribute bounds, links, asset identity, document bytes/depth, and derived text size. Authoritative normalized plain text comes from the validated document, including meaningful table/task-list separation, not from an untrusted frontend text snapshot.

Rich content is not arbitrary HTML. It cannot load a remote image, reference an arbitrary local file, execute scripts, or evaluate a template expression. Frontend editor/renderer configuration and native validation need parity, but the native boundary remains authoritative.

Sources: [rich-content validator](../../src-tauri/src/cards/content.rs), [editor contract](../../src/editor/adapter.ts), [rich-editor reference](rich-document-editor.md).

## Basic and Reverse

A Basic note's question and answer form a forward presentation. Enabling Reverse creates/activates a stable reverse presentation with independent schedule state. Source edits affect both directions because the note is shared; reviewing one direction does not implicitly review the other.

Removing/restoring a direction must follow existing lifecycle rules and preserve the intended stable identity/history rather than casually creating a new card for the same direction. Move/copy/delete and duplicate operations account for a shared source note instead of accidentally applying the same note mutation twice to a forward/reverse selection.

## Fill in the blank

Cloze content marks bounded text ranges/groups in the note. The presentation mode chooses one-at-a-time or all-together hiding. There is **one stable scheduled card**, not a new schedule for every blank or every change of mode.

A session records the selected group/presentation snapshot so reveal, rerender, or resumption does not silently change the question. Changing source blanks or their rendering mode must not be treated as automatic new learning history. Native validation preserves meaningful, supported cloze structure; generation cannot invent a cloze phrase absent from its required source excerpt.

## Task lists and answer keys

Ordinary task lists preserve their authored checked state. Answer-key lists use shared stable item identities across the two sides of a Basic note. **Copy checklist to answer** repairs missing/duplicate identities, copies the list to the answer, clears persisted question-side boxes, preserves author-marked correct answers, and replaces prior answer-key lists idempotently.

The native validator rejects mismatched paired lists, missing/duplicate IDs, checked question-side choices, or a key with no correct answer. Readers display disabled empty question boxes and reveal the authored key after flipping. The task-list storage contract is not an arbitrary interactive assessment engine.

## Templates are authoring definitions

Templates contain named fields and safe rich layout definitions. Native structural substitution distinguishes inline values from block-rich content and validates the final rendered document. Preview and generation use the same renderer so a UI preview cannot quietly promise output the commit path rejects.

Generated outputs become independent ordinary notes/cards. Editing or deleting the template later does not rewrite those cards or their schedules. The saved template may be backed up as authoring data, but unrelated templates do not need to accompany a portable deck whose generated content is already independent.

Template fields/layouts are data, not JavaScript, SQL, shell, or arbitrary executable HTML. The editor adapter exposes the supported commands without passing a Tiptap editor instance throughout feature screens.

## Duplicates, drafts, and mutations

Duplicate detection uses a candidate fingerprint followed by normalized-content verification. Manual entry requires deliberate override for an exact duplicate. Bulk entry, template generation, and generated JSON import skip exact duplicates according to their native policy. Image-only notes are not duplicate solely because searchable text is empty.

Persistent drafts have stable identities/revisions and can own staged images. A draft is not a committed card. Before commit the latest exact draft revision is flushed; native code validates it and coordinates canonical records, asset promotion, and draft retirement in the use case. Conflict/failure leaves recoverable work. A lost committed response must not lead to a second targetless batch.

Moving preserves note/card identity, schedule, and retained review evidence under the existing current-ownership rules. Copying creates independent content with fresh local study state. Retained review events preserve historical snapshots; finalized deletion/purge explicitly removes history owned by deleted material after its recovery window. Archiving retains that evidence but removes ordinary study availability.

A revision checks freshness, an operation ID identifies a retryable intent, and a transaction protects the related mutation. They are complementary, not interchangeable. A committed write remains successful if a later UI refresh fails.

## Scheduling and history

`card_schedules` is mutable current state. `review_events` is append-only evidence while its owning material is retained. `scheduler.rs` adapts `rs-fsrs`; React displays native previews rather than computing a second authoritative schedule.

A review validates the session/card/submission and previewed schedule revision, records the event, updates the schedule, and advances session progress transactionally. Suspend/bury/reset/reschedule are administrative actions; they do not fabricate review outcomes. Resetting current scheduling does not imply erasing retained review history.

Studyability includes lifecycle, availability, scope, calendar, and applicable daily limits—not only due timestamps. Current workload, current inventory, historical review totals, and remembered percentages are different read models and must not be used as substitutes for each other.

## Search and read projections

FTS5 queries and allowed filters are native-owned with bound values. List projections are bounded and avoid hydrating entire rich documents/session histories for counts. Search results represent presentations, so reverse cards can appear separately while a multi-blank note remains one scheduled result.

Select visible resolves loaded rows; select all matching resolves the complete current query scope deliberately. Bulk source operations deduplicate shared notes. Current tag links use stable IDs; historical-only labels are not secretly current entities. An unavailable filter must be explicitly cleared instead of silently widening the query.

## Interchange and recovery

| Contract | Meaning |
| --- | --- |
| `.studydeck` | One portable content/asset graph, no donor schedule or review history; fresh local schedules on import |
| `.ostudy.json` | Strict content-only authoring input, whole-file validation then one transaction; no arbitrary internal IDs or history |
| `.ostudybackup` | Coherent personal library plus required managed files, including history, templates, documents/guide graph and supported drafts; no credentials |
| Device sync | Explicit authenticated one-way offer/merge of its allowed study graph; not every current database table |
| UI checkpoint | Device-local navigation/session hint, not learner-content interchange |

Each has its own version and validation policy. A database migration number does not determine every external format version. Released migrations/checksums and supported readers are not stale merely because UI controls or parser names changed.

For exact schemas use [migrations](../../src-tauri/migrations/), native type/reader definitions, the [generated JSON reference](../reference/generated-json-import.md), [backups guide](../user-guide/backups-imports-and-sharing.md), and [sync architecture](device-sync.md). AI-produced or imported validity proves a data contract, not factual correctness of study material.

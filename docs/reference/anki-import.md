# Anki deck import

The Anki adapter is a one-way input boundary. It does not embed Anki, Python, Qt, a browser renderer, or add-on code. It uses the existing ZIP, SQLite, JSON, hashing, native-content validation, and managed-image dependencies; this feature adds no Cargo dependencies.

Implementation and qualification are separate: the pull request and exact-head Actions results record whether this source has passed static, fast, Linux, and Android checks. This reference defines supported behavior, not a claim of completed physical-device acceptance.

## Terminology and conversion

Anki **notes** contain named fields. A **note type** defines field order and question/answer templates. **Cards** identify which template or cloze ordinal is actually studyable. The adapter follows the package's `cards` table rather than generating every possible template, preserving optional reverse-card generation and avoiding unused presentations.

A supported Anki card becomes an independently scheduled oStudy presentation with a fresh schedule. Multiple cards generated from one Anki note do not retain linked Anki editing. Ordinary Basic/reverse presentations become editable oStudy Basic notes. Compatible simple Anki cloze presentations become native **Fill in the blank** notes; each source Anki cloze card receives an independent oStudy cloze note/schedule so importing c1/c2 does not accidentally collapse distinct source schedules. Complex cloze templates that contain extra answer fields or media fall back to a rendered Basic card rather than silently discarding information.

For ordinary Basic cards, Anki's common `{{FrontSide}}` back-template expansion is treated as presentation, not answer data. oStudy stores an answer-only back. The learner can enable **Settings → Appearance → Display both question and answer when card is flipped** when they want the front repeated after reveal. Native Fill in the blank imports carry their blank context in the revealed content automatically.

Source subdeck paths are retained as `Anki deck::...` tags in one imported deck, not reconstructed as folders. Tags follow oStudy's 32-per-note and 64-character limits, with omissions/truncation reported. Import does not overwrite an existing deck.

## Supported package profile

The first bounded compatibility profile is `.apkg` containing an uncompressed schema-11 SQLite `collection.anki2` or `collection.anki21` and the JSON media map. If both SQLite names exist, `collection.anki21` is preferred. A modern `collection.anki21b` is rejected before considering any legacy compatibility-placeholder database.

Unknown schemas, newer compressed/protobuf collections, collection backups, and arbitrary standalone Anki databases are not supported. Export an **Anki Deck Package (.apkg)** from Anki with **Support older Anki versions** and **Include media** enabled when necessary. Renaming a package does not convert it.

Supported template behavior includes ordinary named fields, nested `#`/`^` field conditionals, `FrontSide`, `text:`, `type:`, ordinary `cloze:` replacements, repeated deletions with the same ordinal, cloze hints, and the Tags/Type/Deck/Subdeck/Card built-in values. `type:` preserves the study content as a normal reveal-answer card; Anki's interactive typing/comparison widget is not imported. Substituted field text is never reparsed as a template. Missing fields, unknown filters, nested cloze, and unsupported card behavior produce skipped-card details rather than silently wrong answers.

Supported HTML is converted into application-owned Tiptap JSON: text, paragraphs, common emphasis/subscript/superscript/code marks, line breaks, separators, and managed PNG/JPEG/GIF/WebP images. Headings, lists, tables, and links may be simplified to readable text. Arbitrary CSS and exact layout are not retained. A package stylesheet or card-local CSS is counted as a **native appearance conversion** in the aggregate report instead of emitting thousands of duplicate per-card warnings. The importer is not a standards-complete browser renderer or Anki template engine.

### JavaScript salvage

Anki templates can contain arbitrary JavaScript. oStudy never executes imported JavaScript and does not attempt to emulate a browser runtime. A `<script>` element or inline event handler such as `onclick=` is therefore treated as untrusted executable presentation behavior, not as native card data.

The converter now uses a conservative static-salvage rule instead of treating every script-bearing card as inherently unusable. Script elements are removed as inert source text, inline event-handler attributes are discarded, and the remaining HTML is converted normally. If both the resulting question and answer still contain meaningful static study content (text or a managed image), the card is imported and counted in the aggregate **JavaScript recovered as static content** total. No script is evaluated, no DOM APIs are provided, and no network content is fetched.

If removing JavaScript leaves either side without meaningful study content, the card is skipped with an explicit compatibility detail because its learning meaning may have depended on dynamic generation. Other still-active constructs such as iframes, buttons/forms, hidden CSS-dependent content, audio/video, or unsupported embedded objects continue to fail closed rather than being guessed at. Static salvage is therefore intentionally one-way and conservative: it can preserve obviously complete question/answer content around cosmetic scripts, but it does not claim to prove that arbitrary JavaScript was semantically unnecessary.

## Extension boundary for future native card types

Package/container parsing is intentionally separate from semantic conversion. `package.rs` and `collection.rs` answer **what is in the Anki package**; they do not decide which oStudy authoring type should represent it. `template.rs` answers **what the source presentation means** within the supported template subset. `mod.rs` maps that source presentation into a native oStudy type through the `SourcePresentationKind` conversion boundary.

That separation is deliberate for future card types. When oStudy gains native Image Occlusion or richer cloze/fill-blank authoring, the importer should add a source-presentation variant and converter that produces that native type. It should not teach ZIP/SQLite/media code about UI-specific behavior, and it should not flatten a newly representable card to Basic merely because the first importer did so. Unsupported source semantics should continue to fail or fall back explicitly until the native representation can preserve their learning meaning.

The same rule applies to scheduling: imported cards start with oStudy schedules. A future native multi-presentation note type must decide explicitly whether Anki's source cards map one-to-one to native schedules or share a note-level authoring object. Linked editing and schedule identity are separate concerns.

## Module boundaries

| Module | Responsibility |
| --- | --- |
| `src-tauri/src/studydeck/anki/package.rs` | Bounded ZIP/container/media-map reading; local media resolution; no path extraction |
| `src-tauri/src/studydeck/anki/collection.rs` | Read-only schema-11 SQLite metadata, notes, and existing cards |
| `src-tauri/src/studydeck/anki/template.rs` | Bounded nonrecursive field/filter evaluation; cloze interpretation |
| `src-tauri/src/studydeck/anki/html.rs` | Conservative HTML-to-native-node conversion; script/event-handler stripping; active/hidden-content rejection; native-style detection |
| `src-tauri/src/studydeck/anki/mod.rs` | Source-presentation classification, native type/package graph construction, tags, progress snapshots, compatibility report |
| `src-tauri/src/studydeck/anki/tests.rs` | Generated, redistributable SQLite/ZIP fixtures and native import regressions |

The adapter returns `PackageData` to the existing package validator and transactional importer. It does not introduce a second library writer, an Anki database migration, a separate asset lifecycle, or a new frontend parsing subsystem. Native external-file staging recognizes the container, and the normal `import_deck` command performs conversion and import. React receives only validated results and a bounded report.

The command response adds optional `ankiReport` and `postCommitWarning` fields while retaining the existing top-level deck/card/asset fields. Additive report counters are optional at the TypeScript boundary so older consumed-transfer receipts remain readable. Anki imports use the staged-file identity for transactional replay; a receipt-storage error after the database commits is reported as committed success with a warning, not as a failed import inviting a duplicate write.

## Live import progress

Anki import runs on the blocking worker rather than the UI thread. The backend publishes a lightweight `ostudy-operation-progress` event keyed by the existing import `operationId`; the payload has a schema version, operation kind, phase, optional processed/total counts, and succeeded/failed counters. The event is advisory UI state only and never participates in transaction success or replay identity.

The Anki adapter reports **Reading package → Converting cards → Importing media → Saving deck**. Once the collection is known, total cards are available and conversion progress is published every 100 processed cards plus the final card. The import dialog displays the card count and imported/skipped totals with the shared Progress primitive. A progress-listener failure cannot fail an otherwise valid import. This event contract is intentionally operation-oriented rather than Anki-widget-specific so future long-running workflows can reuse the same shape.

## Resource and trust limits

Archive input is limited to 640 MiB, declared expanded archive content to 768 MiB, and archive entries to 100,004. The collection database is limited to 128 MiB, the JSON media map to 8 MiB, each image to 25 MiB, and cached imported image bytes to 256 MiB. Unused images are excluded from the native package.

The collection reader permits at most 100,000 notes and 100,000 cards, 1,000 note types, 10,000 deck definitions, 128 fields per note type, and 128 MiB of aggregate source note text. It requires ordinary tables, not views, virtual tables, or generated/hidden columns. Queries are fixed application SQL; imported SQL statements are never executed.

Templates/rendered sides are limited to 256 KiB, template nesting to 32 levels, replacement parsing to 4,096 tokens, HTML to 20,000 tokens per side, and converted native document text to an aggregate 128 MiB. The existing native content validator applies its own additional bounds. These are size/work guards, not a promise of constant-time conversion or suitability for every mobile memory budget.

The report retains total imported/skipped/content-simplified counts plus aggregate native-style, typed-answer, native-cloze, cloze-fallback, and JavaScript-static-salvage counts. It keeps at most 100 issue details, each limited to 300 characters, and records the number of omitted details. Repetitive CSS/native-appearance and successful JavaScript-salvage conversions do not consume those 100 issue slots. The dialog keeps this post-commit report visible until closed or the imported deck is opened. An all-incompatible package creates no deck.

## Acceptance criteria and regression coverage

- **AC-01:** Compatible `.apkg` uses native file staging and contextual deck import, without external converters for supported packages.
- **AC-02:** Actual Basic/reverse/optional-reverse presentations, typed-answer content, compatible native cloze cards, tags, readable text, and supported local images become native cards with fresh schedules.
- **AC-03:** Script-bearing cards with complete static question/answer content are salvaged without executing code; dynamically generated/incomplete or still-active content fails at explicit boundaries.
- **AC-04:** Unsafe/unsupported archives, ambiguous media maps, unknown schema, and unsupported template behavior fail at explicit boundaries.
- **AC-05:** Native transaction rollback/replay, managed assets, bounded visible compatibility reporting, live operation progress, and post-commit truth are preserved.
- **AC-06:** Generated fixtures, native/React regression tests, and maintained user/reference documentation accompany the feature.

Focused native coverage resides under `studydeck::anki`; frontend coverage is in `src/controllers/deckTransferController.anki.test.ts`, `src/screens/flashcards/DeckTransferDialog.anki.test.tsx`, and practice reveal tests. The native fixture exercises actual optional-reverse/card ordinals, two native cloze presentations, a typed-answer card, a managed image, a statically salvageable scripted card, a script-generated card that must still skip, progress phases/counters, missing media, modern-package rejection, unsafe paths, duplicate media entries, unsupported schemas/views, native transaction replay/rollback, fresh schedules, old receipt compatibility, and report limits.

Use the canonical static/fast/full and affected Android gates in the development runbook. Freeze source and documentation before terminal qualification; do not rerun expensive gates for unrelated documentation edits. Human acceptance should import representative compatible decks on desktop and Android, watch card progress advance, inspect report totals/images, compare Basic and cloze reveals with Anki, spot-check statically salvaged JavaScript cards against their source, open/edit/practice cards, cancel file selection, and verify that an unsupported package creates no deck.

## Add-on-inspired follow-ups

**Review Heatmap:** a native calendar could show the learner's daily review activity, clickable days, and separately labelled due forecasts. Use oStudy review events and local-calendar semantics. Do not import another learner's streak or copy an add-on implementation.

**Contextual study-notes sidebar:** locally saved questions to revisit could link to the current card or document without leaving Practice. These study notes are distinct from Anki notes (field records that generate cards). Treat this as a separate feature with explicit persistence and synchronization scope rather than embedding an Anki add-on.

## Primary references

- [Anki key concepts](https://docs.ankiweb.net/getting-started.html#key-concepts)
- [AnkiDroid database structure, schema 11](https://github.com/ankidroid/Anki-Android/wiki/Database-Structure)
- [Card generation](https://docs.ankiweb.net/templates/generation.html)
- [Field replacements](https://docs.ankiweb.net/templates/fields.html)
- [Styling and HTML](https://docs.ankiweb.net/templates/styling.html)
- [Exporting](https://docs.ankiweb.net/exporting.html)
- [Current package metadata and media-map definitions](https://github.com/ankitects/anki/blob/main/proto/anki/import_export.proto)
- [Review Heatmap feature description](https://github.com/glutanimate/review-heatmap/blob/main/docs/description.md)

These references describe formats and feature inspiration. No Anki or add-on implementation code is incorporated.

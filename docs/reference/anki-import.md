# Anki deck import

Status: implementation contract for the Anki import development branch. This document does not claim that unpublished code or unrun qualification is complete.

## Design

Anki is an input format, not a runtime dependency. The adapter reads a bounded ZIP package, opens its extracted SQLite collection read-only, evaluates a deliberately limited template language, and converts the resulting question/answer presentations into validated native Tiptap JSON. It does not embed Anki, Python, Qt, a browser renderer, or add-on code.

Anki notes contain named fields. Note types define field order and question/answer templates. Cards identify which template or cloze ordinal is actually studyable. Import follows the package's cards table rather than synthesizing cards from every template: this preserves optional reverse-card generation and avoids importing unused templates. Arbitrary generated presentations become independent editable Basic notes in ostudy; they do not retain linked Anki note editing. Scheduling and review history start fresh.

The first bounded compatibility target is `.apkg` containing an uncompressed schema-11 `collection.anki2` or `collection.anki21` and the JSON media map. Modern `collection.anki21b`, unknown schemas, and collection backups are rejected with actionable export instructions rather than accidentally importing a legacy compatibility placeholder. Export an Anki Deck Package with **Support older Anki versions** and **Include media** enabled when necessary.

Subdeck names remain available as source-deck tags in the imported deck. This conversion does not reconstruct the Anki deck tree or overwrite existing decks. Basic, reverse, optional reverse, ordinary field substitution, nested field conditionals, `FrontSide`, `text:` and cloze replacements are in scope. Unsupported filters and executable templates must produce a compatibility report, never silently wrong answers.

Images supported by ostudy are copied into its existing managed asset lifecycle. CSS is not executed or retained as arbitrary HTML. Audio, video, JavaScript, TTS, image-occlusion add-ons, and Anki scheduling are outside the first import contract. The report must distinguish skipped cards from downgraded formatting and omitted media. Nothing is fetched from the network during conversion.

## Acceptance criteria

- **AC-01:** Import a supported `.apkg` from the existing contextual deck-import workflow, on the native file-staging boundary, without Anki libraries or a separate conversion program.
- **AC-02:** Convert actual Basic/reverse/optional-reverse/cloze card presentations and tags; preserve readable rich text and supported local images; start with fresh schedules.
- **AC-03:** Bound archive/database/template/media work, reject unsafe archives and unsupported collection variants, and never execute imported scripts, CSS, SQL statements, or network requests.
- **AC-04:** Reuse the native transactional, retry-safe deck-import and managed-asset machinery; return a visible, bounded compatibility report and keep committed success distinct from refresh failures.
- **AC-05:** Add generated, redistributable fixtures/regression tests and update the maintained import documentation before terminal qualification.

## Future feature ideas, not part of this importer

Review Heatmap suggests a native calendar showing daily review activity, clickable days, and separately labelled due forecasts. It should use ostudy's review events and calendar semantics, not import another user's streak or Anki add-on implementation.

A contextual study-notes sidebar could retain questions to revisit, link them to cards/documents, and save locally without leaving Practice. This is distinct from an Anki note (the source fields that generate cards). It belongs in a separate product change rather than this format adapter.

## Primary references

- [Anki key concepts](https://docs.ankiweb.net/getting-started.html#key-concepts)
- [AnkiDroid database structure, schema 11](https://github.com/ankidroid/Anki-Android/wiki/Database-Structure)
- [Card generation](https://docs.ankiweb.net/templates/generation.html)
- [Field replacements](https://docs.ankiweb.net/templates/fields.html)
- [Styling and HTML](https://docs.ankiweb.net/templates/styling.html)
- [Current package metadata and media-map definitions](https://github.com/ankitects/anki/blob/main/proto/anki/import_export.proto)
- [Review Heatmap feature description](https://github.com/glutanimate/review-heatmap/blob/main/docs/description.md)

These references describe the format and feature inspiration. No Anki or add-on implementation code is incorporated.

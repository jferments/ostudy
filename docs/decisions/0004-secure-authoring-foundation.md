# ADR 0004: Treat card content and shared decks as untrusted data

- **Status:** Accepted
- **Applies from:** Phase 2 Stage 1 / database schema 4

## Context

Cards and templates can be typed locally or imported from a shared deck. A
shared deck may contain intentionally hostile strings or a deliberately crafted
SQLite file. The application must support rich text, Fill in the blank markers,
field templates, tags, and full-text search without allowing card authors to
execute JavaScript, inject HTML, alter SQL, read arbitrary files, or smuggle
unsupported SQLite objects into the library.

The application also needs one editable note to generate multiple studyable
cards. Reverse notes, Fill in the blank notes, and field templates all rely on
that note/card separation.

## Decision

1. **Rich card content remains validated JSON, not HTML.** Rust accepts only the
   versioned Tiptap/ProseMirror node, mark, attribute, link, and managed-image
   allowlists already defined by the card validator. Text is serialized as JSON
   text nodes and rendered through React/Tiptap rather than injected with
   `innerHTML`.
2. **Templates are deliberately non-executable.** A template is literal text
   plus exact `{{field name}}` placeholders. Field names use a closed grammar;
   malformed braces, property access, expressions, and unknown fields are
   rejected. Rendering performs literal string substitution and then creates
   canonical JSON text nodes.
3. **Fill in the blank is also a literal grammar.** The only special syntax is
   `[[answer]]`. Nested, empty, or unmatched markers are rejected. Each marker
   creates one generated card.
4. **Rust owns all validation and transactions.** Frontend checks improve the
   editing experience but are never authoritative. SQL uses bound parameters.
   User-provided strings are never concatenated into SQL statements.
5. **Search input is converted to a safe FTS expression.** The backend extracts
   ordinary word tokens, quotes each token, applies prefix matching, and binds
   the resulting expression as a parameter. User-entered FTS operators are not
   interpreted as operators.
6. **Shared decks use a versioned SQLite package with an exact schema.** Import
   opens the package read-only, disables `trusted_schema`, performs integrity
   and size checks, rejects extra SQLite objects or columns, validates every ID,
   relationship, rich document, template, tag, and image, and then inserts the
   content in one local transaction. Review history and local settings are not
   transferred.
7. **Generated cards are materialized identities.** Notes store editable source
   data; cards store direction/ordinal/display identity and independent schedule
   rows. Rendering a study card is done from the selected card ID so reverse,
   Fill in the blank, and template cards show the correct prompt.

## Consequences

- Malicious card text is displayed as text rather than executed.
- Template capabilities are intentionally narrower than a general scripting
  language, but the format remains understandable, portable, and auditable.
- Import code is more verbose because it validates the complete SQLite package
  boundary before trusting any row.
- Changing a template that is already used by notes is blocked in Stage 1 so
  existing generated cards cannot silently change meaning.
- Future template features must extend the literal grammar and package version
  explicitly; they must not introduce arbitrary JavaScript, HTML, CSS, or SQL.

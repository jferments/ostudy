# ADR 0004: Keep templates and shared decks non-executable

- **Status:** Accepted
- **Date:** 2026-07-31
- **Applies from:** database schema 4 / Phase 2

## Context

Phase 2 adds field-based card templates, Fill in the blank generation, FTS5
search, and richer SQLite deck exchange. All four features process content that
may come from a shared deck. Allowing templates to evaluate code, allowing card
HTML to execute, concatenating search text into SQL, or trusting an imported
SQLite schema would turn ordinary study content into an application-code or
local-data attack surface.

The application must still support useful reusable templates, rich cards,
search, and one-file deck exchange without requiring a sandboxed scripting
runtime or a second interchange format.

## Decision

### Templates

The template language recognizes only exact `{{field name}}` placeholders.
Fields and card specifications are bounded and validated. There are no
expressions, loops, conditions, includes, property access, function calls,
JavaScript, SQL, HTML interpretation, or style language. Substitution is literal
and rendered into validated Tiptap JSON by Rust.

Fill in the blank is a separate literal `[[answer]]` grammar with no general
evaluator.

### Rich content

Canonical card content remains versioned Tiptap/ProseMirror JSON. Rust uses
explicit allowlists for nodes, marks, attributes, links, image IDs, predefined
sizes, and alignments. The frontend does not inject persisted card strings as
HTML.

### Search and SQL

All SQL values use parameters. FTS search input is tokenized and quoted so user
input cannot provide an FTS expression or database statement.

### Shared deck packages

A version 2 `.studydeck` is a narrowly specified SQLite database. Import opens
it read-only, disables trusted schema, checks integrity, requires the exact
schema, rejects views/triggers/extra objects, enforces row and byte limits, and
validates relationships, templates, documents, and assets before one local
transaction. Duplicate keys and local IDs are recomputed or remapped. Personal
schedules, reviews, settings, drafts, and state history are never imported.

A legacy version 1 importer remains only as a compatibility fallback.

## Consequences

- Shared templates are intentionally less expressive than general templating
  engines, but are inspectable and sufficient for structured study records.
- Template behavior is deterministic and can be tested without a script sandbox.
- New rich-content nodes, URL schemes, image attributes, or package columns
  require explicit validator, migration, test, and documentation changes.
- A package that is almost valid but contains additional schema objects is
  rejected rather than partially trusted.
- SQLite remains the only deck interchange container; CSV/TSV is not needed for
  Phase 2.
- Future math support must define its own safe storage and renderer rules rather
  than embedding arbitrary executable markup.

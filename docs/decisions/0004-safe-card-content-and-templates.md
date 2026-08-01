# ADR 0004: Treat cards, deck packages, and templates as untrusted data

- **Status:** Accepted
- **Applies from:** Phase 2, Stage 1

## Context

Decks can be shared between users. Card text, formatting, links, images, and
future template fields therefore cross a trust boundary. Rendering imported
HTML or evaluating template expressions would allow a malicious deck to attempt
script execution, local-file access, remote tracking, command execution, or
unsafe database operations.

## Decision

Card content remains a versioned structured document validated by Rust before it
is stored or rendered.

- The document schema uses explicit node, mark, attribute, nesting, and size
  allowlists.
- Unknown content is rejected rather than passed through.
- Links are restricted to validated `http`, `https`, `mailto`, and `tel` URLs.
- Images are managed local assets identified by stable IDs; cards cannot supply
  arbitrary local paths or remote image URLs.
- Rust derives durable plain text from the validated document.
- User values are bound as SQLite parameters rather than interpolated into SQL.
- The WebView receives validated structured data, not raw imported HTML.

The future field-template language is data-only. It may define field names,
field values, and deterministic card layouts. It may not execute JavaScript,
SQL, shell commands, raw HTML, filesystem operations, network requests, or a
general-purpose expression language. Generated documents pass through the same
Rust validator as manually authored and imported cards.

## Consequences

- Shared decks cannot add arbitrary executable behavior.
- New document features require an explicit schema and validator update.
- Imports created by newer unsupported versions fail safely instead of silently
  discarding or executing unknown content.
- Template functionality is intentionally less expressive than a scripting
  language, but is predictable, portable, and auditable.

# ADR 0002: distinguish unsupported rich content from malformed rich content

- **Status:** Accepted
- **Date:** 2026-07-28

## Context

Card content is persisted as versioned Tiptap/ProseMirror JSON and can arrive
from the local editor or an imported `.studydeck` package. Validation failures
have two materially different meanings:

1. the document uses a node/mark that this application version does not support;
2. the document uses known nodes/marks in a malformed or illegal structure.

The initial validator checked a parent's child-type relationship before
recursively validating each child. An unknown `iframe` child therefore failed
as a generic malformed document before the validator reached the more precise
"unsupported node" check.

## Decision

Each child node is validated before the parent/child relationship is checked.
The validator uses these stable classifications:

- `unsupported_card_content`: unknown node or formatting mark, normally
  indicating content from a newer or incompatible schema;
- `invalid_card_document`: malformed JSON or an illegal arrangement of known
  nodes;
- `invalid_image_reference`: an image that does not use a valid managed local
  asset reference;
- specialized codes such as `invalid_link` and `invalid_card_style` for
  actionable user-facing errors.

## Consequences

- Import errors can distinguish incompatible content from corrupted content.
- The error contract is more useful to future schema migration code.
- Validation order is now behavior and is protected by regression tests.
- New schema nodes/marks require an explicit validator update and schema-version
  decision rather than being silently accepted.

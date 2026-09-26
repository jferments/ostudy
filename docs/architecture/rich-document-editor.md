# Rich-document editor boundary and schema versions

## Contract and current consumers

Rich authoring uses `EditorAdapter`, `EditorCapabilities`, `EditorCommandRequest`, and canonical `RichDocument`. Features using the rich editor do not receive a Tiptap editor instance or construct private extension command chains. The current implementation is under `src/editor/tiptap/`; `src/editor/adapter.ts` is the reusable contract and `src/editor/tiptapAdapter.ts` implements it.

Toolbar groups follow advertised capabilities. Cards and templates consume the rich editor; renderers consume its validated structured content. **The current Study Guide reader uses plain-text textarea entry for notes/section overrides backed by rich-document storage.** A reusable adapter or schema capability is not proof that the full rich toolbar is exposed in every feature. Extending that UI would be separate product work.

## Independent schema versions

Canonical roots support `attrs.schemaVersion` **1 through 3**. The existing database `content_schema_version` remains the released `tiptap-json` transport-envelope version 1; the canonical rich-root version is independent.

Version 2 adds mutually exclusive subscript/superscript marks; table/row/header/cell nodes; task lists/items; ordinary or answer-key task-list mode; checked state and stable item IDs; and validated table span/column-width attributes. Version 3 adds typed inline/block mathematics plus managed local audio/video atoms. The native schema-3 admission layer validates those atoms, projects only the validation view onto the schema-2 structural validator, and stores the original schema-3 document unchanged.

Rust validates root/version, nodes/marks/attributes, parent-child structure, table shape, checklist identities/mode, image/media references, links, style bounds, size, depth, math source and managed-media identity. It derives authoritative searchable text rather than trusting a frontend plain-text snapshot. See [card/data model](card-format-and-data-model.md), [schema-3 admission](../../src-tauri/src/cards/rich_content.rs), and the underlying [structural validator](../../src-tauri/src/cards/content.rs).

## Tables

Editable cells are keyboard-reachable. Supported commands insert a table, toggle its header row, add/remove rows and columns, and delete the table. Compact layouts allow local horizontal table scrolling rather than shrinking text or causing document-level overflow.

Cell merge/split is intentionally not advertised. Safe rectangular selection and span normalization need a complete interaction model; partial support risks corrupting persisted shape. Native validation still checks the spans and widths that existing supported content can contain.

## Ordinary and answer-key task lists

Ordinary task lists retain authored checked state through editing, reading, practice, interchange, and backup.

Answer-key lists pair stable item identities across a Basic note's question and answer. **Copy checklist to answer** repairs missing/duplicate IDs, copies the complete checklist, clears persisted question boxes, keeps the authored correct boxes on the answer, and replaces previous answer-key lists so repeated use is idempotent.

Rust rejects differing paired lists, absent/duplicate IDs, checked question boxes, and answer keys with no correct option. The question therefore renders disabled empty boxes and the answer reveals the key. This is a safe authored card pattern, not arbitrary executable quiz markup.

## Formatting organization

Frequently used formatting remains direct: bold, italic, underline, subscript, superscript, lists, and task lists. Insert contains tables, answer checklists, and horizontal rules. Advanced formatting contains less frequent undo/redo, strike, inline code, blockquote, and code-block operations. Contextual table controls appear only inside a table.

Font family/size, alignment, indentation, text color, and highlight continue through adapter commands and native style bounds. Template field placeholders are a supported private-adapter/canonical-template feature, not arbitrary provider HTML or generic executable expressions.

## Images, links, and lifecycle

Images enter through managed asset acquisition and become controlled `study-asset://` identities. Editors must not persist arbitrary desktop paths, Android content URIs, remote image URLs, or raw provider image locations. Draft-owned staged images and canonical committed images have separate native lifecycles.

Allowed links and structured formatting are validated at save/import boundaries. A safe editor implementation is not a reason to trust imported JSON without native validation. A draft autosave and a committed card/template are different results; exact draft revision, canonical transaction, asset promotion, and retry identity must stay coherent.

## Deferred capabilities

Mathematics and managed audio/video are current typed schema-3 atoms; raw provider HTML is not a shortcut. Further math/media expansion still requires native/PDF/accessibility/interchange parity. Find/replace across tables/checklists needs dedicated selection semantics. A persistent character counter should follow an actual authoring need rather than clutter every screen; native bounds already apply.

Future backend replacement must preserve schema compatibility and adapter behavior across card, template, read-only, draft, interchange, and native validation tests. It does not require exposing third-party editor types to every consumer or changing SQLite to match toolbar layout.

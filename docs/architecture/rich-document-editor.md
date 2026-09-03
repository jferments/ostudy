# Rich-document editor boundary and schema version 2

## Purpose

The public authoring surfaces depend on `EditorAdapter`, `EditorCapabilities`,
`EditorCommandRequest`, and canonical `RichDocument`. They do not receive a
Tiptap editor instance, import Tiptap extension names, or construct command
chains. The current Tiptap implementation lives under `src/editor/tiptap/` and
may be replaced without changing card, template, Study Guide, or practice
feature contracts.

`src/editor/adapter.ts` is the reusable contract and
`src/editor/tiptapAdapter.ts` is the concrete implementation. Toolbar groups are
created from declared capabilities so adding or removing an editor backend does
not silently leave unsupported buttons in feature screens.

## Canonical rich-document schema

The document root carries `attrs.schemaVersion = 2`. Version 2 adds:

- `subscript` and `superscript` marks, which are mutually exclusive;
- `table`, `tableRow`, `tableHeader`, and `tableCell` nodes;
- `taskList` and `taskItem` nodes;
- `taskList.attrs.mode`, either `ordinary` or `answer_key`;
- `taskItem.attrs.checked` and an optional stable `itemId`;
- table `colspan`, `rowspan`, and `colwidth` attributes.

Rust remains authoritative. It validates the version, every node and mark,
parent/child structure, spans, widths, checklist modes, item identities, image
references, links, and style bounds before writing. Authoritative search text is
derived from the validated document, including table cell separation and task
checkbox state.

The existing database `content_schema_version` remains the released
`tiptap-json` transport-envelope version. The independently versioned canonical
document root controls rich-node compatibility and allows old schema-1 documents
to continue opening without a destructive data migration.

## Tables

Tables are keyboard-reachable through their editable cells. The editor provides
insert, header-row toggle, add/remove row, add/remove column, and delete-table
commands. Compact readers and editors scroll the table horizontally rather than
shrinking text below a readable size.

Cell merge and split are deliberately not advertised. Correct rectangular cell
selection and span-normalization require a dedicated table-selection model; a
partial implementation would risk corrupting persisted table shape.

## Task and answer-key checklists

Ordinary task lists preserve their authored checked state in editing, card
preview, practice, import/export, and backup.

Answer-key checklists use the same stable item identities on both sides of a
Basic card. The author marks correct choices in the question editor and activates
**Copy checklist to answer**. That operation:

1. repairs missing or duplicate item identities;
2. copies the complete checklist to the answer;
3. clears every persisted checkbox on the question;
4. preserves the marked correct boxes on the answer;
5. replaces previous answer-key lists, making repeated use idempotent.

Rust rejects a save when the question and answer lists differ, identities are
missing or duplicated, a question box is still marked, or an answer list has no
correct option. Therefore the question always presents disabled empty boxes and
the answer reveals the author-marked key after the card is flipped.

## Low-noise formatting

Bold, italic, underline, subscript, superscript, lists, and task lists remain
immediately available. Tables, answer checklists, and horizontal rules live in
an Insert menu. Undo/redo, strike, inline code, blockquote, and code block live
in Advanced formatting. Contextual table controls appear only while the cursor
is in a table.

## Evaluated and deferred additions

- **Mathematics/KaTeX:** deferred. A safe mathematics node needs a typed source
  grammar, sanitizer policy, native/PDF renderer parity, accessibility text, and
  import/export compatibility. Raw provider HTML is not an acceptable shortcut.
- **Typography:** the existing font-family, font-size, alignment, indentation,
  text-color, and highlight controls remain supported through adapter commands.
- **Character count:** deferred until a product-level limit or writing workflow
  makes a persistent counter useful; the native size limits remain enforced.
- **Find/replace:** deferred because browser/editor selection semantics and
  replacement across table/checklist boundaries require a dedicated interaction
  design.
- **Placeholders:** template field placeholders remain supported by the private
  adapter and canonical template schema; generic authoring placeholders are not
  added.

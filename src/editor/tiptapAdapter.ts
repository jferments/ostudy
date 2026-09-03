import type { Editor } from "@tiptap/core";
import { Fragment, type Node as ProseMirrorNode } from "@tiptap/pm/model";

import type { RichDocument } from "../ipc/common";
import {
  RICH_DOCUMENT_SCHEMA_VERSION,
  type EditorAdapter,
  type EditorCapabilities,
  type EditorCommandRequest,
  type EditorSelectionState,
} from "./adapter";
import { ANSWER_KEY_MODE } from "./extensions";
import { canonicalizeDocument } from "./richText";

export const TIPTAP_EDITOR_CAPABILITIES: EditorCapabilities = {
  schemaVersion: RICH_DOCUMENT_SCHEMA_VERSION,
  marks: {
    bold: true,
    italic: true,
    underline: true,
    strike: true,
    inlineCode: true,
    subscript: true,
    superscript: true,
  },
  blocks: {
    headings: true,
    bulletList: true,
    numberedList: true,
    blockquote: true,
    codeBlock: true,
    horizontalRule: true,
  },
  tables: {
    insert: true,
    rows: true,
    columns: true,
    headerRow: true,
    // Cell selections and rectangular spans require a dedicated table-selection
    // implementation. The adapter deliberately does not advertise an unsafe
    // partial merge/split command.
    mergeSplit: false,
  },
  checklists: {
    ordinary: true,
    answerKey: true,
    stableItemIds: true,
  },
  history: { undo: true, redo: true },
};

/** Private concrete-editor adapter. Feature code consumes only EditorAdapter. */
export function createTiptapEditorAdapter(editor: Editor): EditorAdapter {
  return {
    capabilities: TIPTAP_EDITOR_CAPABILITIES,
    selection: () => selectionState(editor),
    document: () => canonicalizeDocument(editor.getJSON() as RichDocument),
    plainText: () => editor.getText({ blockSeparator: "\n" }),
    attributes: (name) => editor.getAttributes(name),
    isActive: (name, attributes) => editor.isActive(name, attributes),
    execute: (request) => execute(editor, request),
    canExecute: (request) => canExecute(editor, request),
    setDocument: (document, emitChange = false) =>
      editor.commands.setContent(document, { emitUpdate: emitChange }),
    setEditable: (editable) => editor.setEditable(editable, false),
    focus: () => {
      editor.commands.focus();
    },
  };
}

function selectionState(editor: Editor): EditorSelectionState {
  const blockStyle = editor.isActive("heading", { level: 1 })
    ? "h1"
    : editor.isActive("heading", { level: 2 })
      ? "h2"
      : editor.isActive("heading", { level: 3 })
        ? "h3"
        : editor.isActive("heading", { level: 4 })
          ? "h4"
          : "paragraph";
  const marks = new Set<string>();
  for (const mark of [
    "bold",
    "italic",
    "underline",
    "strike",
    "code",
    "subscript",
    "superscript",
    "link",
  ]) {
    if (editor.isActive(mark)) marks.add(mark);
  }
  const alignment = ["left", "center", "right", "justify"].find((value) =>
    editor.isActive({ textAlign: value }),
  );
  const taskAttributes = editor.getAttributes("taskItem");
  return {
    blockStyle,
    textAlignment:
      alignment === "left" ||
      alignment === "center" ||
      alignment === "right" ||
      alignment === "justify"
        ? alignment
        : null,
    marks,
    inTable: editor.isActive("table"),
    inTaskList: editor.isActive("taskList"),
    taskChecked:
      editor.isActive("taskItem") && typeof taskAttributes.checked === "boolean"
        ? taskAttributes.checked
        : null,
  };
}

function execute(editor: Editor, request: EditorCommandRequest): boolean {
  switch (request.command) {
    case "toggle_bold":
      return editor.chain().focus().toggleBold().run();
    case "toggle_italic":
      return editor.chain().focus().toggleItalic().run();
    case "toggle_underline":
      return editor.chain().focus().toggleUnderline().run();
    case "toggle_strike":
      return editor.chain().focus().toggleStrike().run();
    case "toggle_inline_code":
      return editor.chain().focus().toggleCode().run();
    case "toggle_subscript":
      return editor.chain().focus().unsetMark("superscript").toggleMark("subscript").run();
    case "toggle_superscript":
      return editor.chain().focus().unsetMark("subscript").toggleMark("superscript").run();
    case "toggle_bullet_list":
      return editor.chain().focus().toggleBulletList().run();
    case "toggle_numbered_list":
      return editor.chain().focus().toggleOrderedList().run();
    case "toggle_task_list":
      return editor.chain().focus().toggleList("taskList", "taskItem").run();
    case "toggle_task_checked":
      return toggleTaskChecked(editor);
    case "toggle_blockquote":
      return editor.chain().focus().toggleBlockquote().run();
    case "toggle_code_block":
      return editor.chain().focus().toggleCodeBlock().run();
    case "set_block":
      if (request.value === "paragraph") return editor.chain().focus().setParagraph().run();
      return editor
        .chain()
        .focus()
        .toggleHeading({ level: Number(request.value.slice(1)) as 1 | 2 | 3 | 4 })
        .run();
    case "set_text_alignment":
      return editor.chain().focus().setTextAlign(request.value).run();
    case "set_font_family":
      return request.value
        ? editor.chain().focus().setFontFamily(request.value).run()
        : editor.chain().focus().unsetFontFamily().run();
    case "set_font_size":
      return request.value
        ? editor.chain().focus().setFontSize(request.value).run()
        : editor.chain().focus().unsetFontSize().run();
    case "set_text_color":
      return editor.chain().focus().setColor(request.value).run();
    case "set_highlight":
      return editor.chain().focus().setHighlight({ color: request.value }).run();
    case "reset_colors":
      return editor.chain().focus().unsetColor().unsetHighlight().run();
    case "indent":
      return indent(editor, request.value);
    case "set_link":
      return editor.chain().focus().extendMarkRange("link").setLink({ href: request.value }).run();
    case "unset_link":
      return editor.chain().focus().extendMarkRange("link").unsetLink().run();
    case "insert_image":
      return editor.chain().focus().setImage(request.value as never).run();
    case "insert_text":
      return editor.chain().focus().insertContent(request.value).run();
    case "insert_horizontal_rule":
      return editor.chain().focus().setHorizontalRule().run();
    case "undo":
      return editor.chain().focus().undo().run();
    case "redo":
      return editor.chain().focus().redo().run();
    case "clear_formatting":
      return editor.chain().focus().unsetAllMarks().clearNodes().run();
    case "insert_table":
      return insertTable(editor, request.value.rows, request.value.columns, request.value.headerRow);
    case "add_table_row":
      return addTableRow(editor);
    case "delete_table_row":
      return deleteTableRow(editor);
    case "add_table_column":
      return addTableColumn(editor);
    case "delete_table_column":
      return deleteTableColumn(editor);
    case "toggle_table_header_row":
      return toggleTableHeaderRow(editor);
    case "delete_table":
      return deleteTable(editor);
    case "insert_answer_checklist":
      return editor
        .chain()
        .focus()
        .insertContent({
          type: "taskList",
          attrs: { mode: ANSWER_KEY_MODE },
          content: request.value.items.map((entry) => ({
            type: "taskItem",
            attrs: { checked: entry.correct, itemId: entry.id },
            content: [
              {
                type: "paragraph",
                content: entry.text ? [{ type: "text", text: entry.text }] : undefined,
              },
            ],
          })),
        })
        .run();
  }
}

function canExecute(editor: Editor, request: EditorCommandRequest): boolean {
  switch (request.command) {
    case "undo":
      return editor.can().chain().focus().undo().run();
    case "redo":
      return editor.can().chain().focus().redo().run();
    case "add_table_row":
    case "delete_table_row":
    case "add_table_column":
    case "delete_table_column":
    case "toggle_table_header_row":
    case "delete_table":
      return findAncestor(editor, "table") !== null;
    case "toggle_task_checked":
      return findAncestor(editor, "taskItem") !== null;
    default:
      return editor.isEditable;
  }
}

function indent(editor: Editor, delta: -1 | 1): boolean {
  if (editor.isActive("taskList")) {
    return delta > 0
      ? editor.chain().focus().sinkListItem("taskItem").run()
      : editor.chain().focus().liftListItem("taskItem").run();
  }
  if (editor.isActive("bulletList") || editor.isActive("orderedList")) {
    return delta > 0
      ? editor.chain().focus().sinkListItem("listItem").run()
      : editor.chain().focus().liftListItem("listItem").run();
  }
  const nodeType = editor.isActive("heading") ? "heading" : "paragraph";
  const current = Number(editor.getAttributes(nodeType).indent ?? 0);
  return editor
    .chain()
    .focus()
    .updateAttributes(nodeType, { indent: Math.min(6, Math.max(0, current + delta)) })
    .run();
}

function insertTable(editor: Editor, rows: number, columns: number, headerRow: boolean): boolean {
  const rowCount = Math.min(20, Math.max(1, Math.trunc(rows)));
  const columnCount = Math.min(12, Math.max(1, Math.trunc(columns)));
  return editor
    .chain()
    .focus()
    .insertContent({
      type: "table",
      content: Array.from({ length: rowCount }, (_, rowIndex) => ({
        type: "tableRow",
        content: Array.from({ length: columnCount }, () => ({
          type: headerRow && rowIndex === 0 ? "tableHeader" : "tableCell",
          attrs: { colspan: 1, rowspan: 1, colwidth: null },
          content: [{ type: "paragraph" }],
        })),
      })),
    })
    .run();
}

interface AncestorContext {
  node: ProseMirrorNode;
  depth: number;
  pos: number;
  index: number;
}

function findAncestor(editor: Editor, name: string): AncestorContext | null {
  const { $from } = editor.state.selection;
  for (let depth = $from.depth; depth > 0; depth -= 1) {
    if ($from.node(depth).type.name === name) {
      return {
        node: $from.node(depth),
        depth,
        pos: $from.before(depth),
        index: $from.index(depth),
      };
    }
  }
  return null;
}

function tableRows(table: ProseMirrorNode): ProseMirrorNode[] {
  return Array.from({ length: table.childCount }, (_, index) => table.child(index));
}

function rowCells(row: ProseMirrorNode): ProseMirrorNode[] {
  return Array.from({ length: row.childCount }, (_, index) => row.child(index));
}

function replaceTable(
  editor: Editor,
  context: AncestorContext,
  rows: readonly ProseMirrorNode[],
): boolean {
  const replacement = context.node.type.create(
    context.node.attrs,
    Fragment.fromArray([...rows]),
  );
  editor.view.dispatch(
    editor.state.tr.replaceWith(context.pos, context.pos + context.node.nodeSize, replacement),
  );
  return true;
}

function makeCell(editor: Editor, header: boolean): ProseMirrorNode | null {
  const type = editor.state.schema.nodes[header ? "tableHeader" : "tableCell"];
  const paragraph = editor.state.schema.nodes.paragraph?.createAndFill();
  if (!type || !paragraph) return null;
  return type.createAndFill(
    { colspan: 1, rowspan: 1, colwidth: null },
    Fragment.from(paragraph),
  );
}

function addTableRow(editor: Editor): boolean {
  const table = findAncestor(editor, "table");
  if (!table) return false;
  const rows = tableRows(table.node);
  const rowIndex = editor.state.selection.$from.index(table.depth);
  const reference = rows[Math.min(rowIndex, rows.length - 1)];
  if (!reference) return false;
  const cells = rowCells(reference)
    .map((cell) => makeCell(editor, cell.type.name === "tableHeader"))
    .filter((cell): cell is ProseMirrorNode => cell !== null);
  if (cells.length === 0) return false;
  const row = reference.type.create(reference.attrs, Fragment.fromArray(cells));
  rows.splice(Math.min(rowIndex + 1, rows.length), 0, row);
  return replaceTable(editor, table, rows);
}

function deleteTableRow(editor: Editor): boolean {
  const table = findAncestor(editor, "table");
  if (!table || table.node.childCount <= 1) return false;
  const rows = tableRows(table.node);
  const rowIndex = Math.min(editor.state.selection.$from.index(table.depth), rows.length - 1);
  rows.splice(rowIndex, 1);
  return replaceTable(editor, table, rows);
}

function addTableColumn(editor: Editor): boolean {
  const table = findAncestor(editor, "table");
  const row = findAncestor(editor, "tableRow");
  if (!table || !row) return false;
  const columnIndex = editor.state.selection.$from.index(row.depth);
  const rows = tableRows(table.node).map((currentRow) => {
    const cells = rowCells(currentRow);
    const reference = cells[Math.min(columnIndex, cells.length - 1)];
    const cell = makeCell(editor, reference?.type.name === "tableHeader");
    if (!cell) return currentRow;
    cells.splice(Math.min(columnIndex + 1, cells.length), 0, cell);
    return currentRow.type.create(currentRow.attrs, Fragment.fromArray(cells));
  });
  return replaceTable(editor, table, rows);
}

function deleteTableColumn(editor: Editor): boolean {
  const table = findAncestor(editor, "table");
  const row = findAncestor(editor, "tableRow");
  if (!table || !row || row.node.childCount <= 1) return false;
  const columnIndex = editor.state.selection.$from.index(row.depth);
  const rows = tableRows(table.node).map((currentRow) => {
    const cells = rowCells(currentRow);
    if (cells.length > 1) cells.splice(Math.min(columnIndex, cells.length - 1), 1);
    return currentRow.type.create(currentRow.attrs, Fragment.fromArray(cells));
  });
  return replaceTable(editor, table, rows);
}

function toggleTableHeaderRow(editor: Editor): boolean {
  const table = findAncestor(editor, "table");
  if (!table || table.node.childCount === 0) return false;
  const rows = tableRows(table.node);
  const first = rows[0];
  if (!first) return false;
  const currentlyHeader = first.childCount > 0 && first.child(0).type.name === "tableHeader";
  const target = editor.state.schema.nodes[currentlyHeader ? "tableCell" : "tableHeader"];
  if (!target) return false;
  const cells = rowCells(first).map((cell) =>
    target.create(cell.attrs, cell.content, cell.marks),
  );
  rows[0] = first.type.create(first.attrs, Fragment.fromArray(cells));
  return replaceTable(editor, table, rows);
}

function deleteTable(editor: Editor): boolean {
  const table = findAncestor(editor, "table");
  if (!table) return false;
  editor.view.dispatch(editor.state.tr.delete(table.pos, table.pos + table.node.nodeSize));
  return true;
}

function toggleTaskChecked(editor: Editor): boolean {
  const item = findAncestor(editor, "taskItem");
  if (!item) return false;
  editor.view.dispatch(
    editor.state.tr.setNodeMarkup(item.pos, undefined, {
      ...item.node.attrs,
      checked: item.node.attrs.checked !== true,
    }),
  );
  return true;
}

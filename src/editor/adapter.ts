import type { RichDocument } from "../ipc/common";

export const RICH_DOCUMENT_SCHEMA_VERSION = 2 as const;

export type EditorBlockStyle = "paragraph" | "h1" | "h2" | "h3" | "h4";
export type EditorTextAlignment = "left" | "center" | "right" | "justify";

export interface EditorTableInsert {
  rows: number;
  columns: number;
  headerRow: boolean;
}

export interface EditorAnswerChecklistInsert {
  items: Array<{ id: string; text: string; correct: boolean }>;
}

export type EditorCommandRequest =
  | { command: "toggle_bold" }
  | { command: "toggle_italic" }
  | { command: "toggle_underline" }
  | { command: "toggle_strike" }
  | { command: "toggle_inline_code" }
  | { command: "toggle_subscript" }
  | { command: "toggle_superscript" }
  | { command: "toggle_bullet_list" }
  | { command: "toggle_numbered_list" }
  | { command: "toggle_task_list" }
  | { command: "toggle_task_checked" }
  | { command: "toggle_blockquote" }
  | { command: "toggle_code_block" }
  | { command: "set_block"; value: EditorBlockStyle }
  | { command: "set_text_alignment"; value: EditorTextAlignment }
  | { command: "set_font_family"; value: string | null }
  | { command: "set_font_size"; value: string | null }
  | { command: "set_text_color"; value: string }
  | { command: "set_highlight"; value: string }
  | { command: "reset_colors" }
  | { command: "indent"; value: -1 | 1 }
  | { command: "set_link"; value: string }
  | { command: "unset_link" }
  | {
      command: "insert_image";
      value: { src: string; assetId: string; alt: string; title?: string };
    }
  | { command: "insert_text"; value: string }
  | { command: "insert_horizontal_rule" }
  | { command: "undo" }
  | { command: "redo" }
  | { command: "clear_formatting" }
  | { command: "insert_table"; value: EditorTableInsert }
  | { command: "add_table_row" }
  | { command: "delete_table_row" }
  | { command: "add_table_column" }
  | { command: "delete_table_column" }
  | { command: "toggle_table_header_row" }
  | { command: "delete_table" }
  | { command: "insert_answer_checklist"; value: EditorAnswerChecklistInsert };

export type EditorCommand = EditorCommandRequest["command"];

export interface EditorCapabilities {
  schemaVersion: number;
  marks: Readonly<{
    bold: boolean;
    italic: boolean;
    underline: boolean;
    strike: boolean;
    inlineCode: boolean;
    subscript: boolean;
    superscript: boolean;
  }>;
  blocks: Readonly<{
    headings: boolean;
    bulletList: boolean;
    numberedList: boolean;
    blockquote: boolean;
    codeBlock: boolean;
    horizontalRule: boolean;
  }>;
  tables: Readonly<{
    insert: boolean;
    rows: boolean;
    columns: boolean;
    headerRow: boolean;
    mergeSplit: boolean;
  }>;
  checklists: Readonly<{
    ordinary: boolean;
    answerKey: boolean;
    stableItemIds: boolean;
  }>;
  history: Readonly<{
    undo: boolean;
    redo: boolean;
  }>;
}

export interface EditorSelectionState {
  blockStyle: EditorBlockStyle;
  textAlignment: EditorTextAlignment | null;
  marks: ReadonlySet<string>;
  inTable: boolean;
  inTaskList: boolean;
  taskChecked: boolean | null;
}

export interface EditorChangeEvent {
  document: RichDocument;
  plainText: string;
  selection: EditorSelectionState;
}

/**
 * Editor-neutral feature boundary. Feature screens may depend on this contract,
 * but never on Tiptap editor objects, extension names, or command chains.
 */
export interface EditorAdapter {
  readonly capabilities: EditorCapabilities;
  selection(): EditorSelectionState;
  document(): RichDocument;
  plainText(): string;
  attributes(name: "link" | "textStyle" | "paragraph" | "heading" | "taskItem"): Record<string, unknown>;
  isActive(name: string, attributes?: Record<string, unknown>): boolean;
  execute(request: EditorCommandRequest): boolean;
  canExecute(request: EditorCommandRequest): boolean;
  setDocument(document: RichDocument, emitChange?: boolean): void;
  setEditable(editable: boolean): void;
  focus(): void;
}

export interface EditorToolbarItem {
  id: string;
  label: string;
  command: EditorCommand;
}

export interface EditorToolbarGroup {
  id: "text" | "lists" | "insert" | "table" | "history";
  label: string;
  items: readonly EditorToolbarItem[];
  advanced: boolean;
}

/** Generates the low-noise toolbar catalog from declared adapter capabilities. */
export function editorToolbarGroups(
  capabilities: EditorCapabilities,
): readonly EditorToolbarGroup[] {
  const groups: EditorToolbarGroup[] = [];
  const text: EditorToolbarItem[] = [];
  if (capabilities.marks.bold) text.push(item("bold", "Bold", "toggle_bold"));
  if (capabilities.marks.italic) text.push(item("italic", "Italic", "toggle_italic"));
  if (capabilities.marks.underline) text.push(item("underline", "Underline", "toggle_underline"));
  if (capabilities.marks.subscript) text.push(item("subscript", "Subscript", "toggle_subscript"));
  if (capabilities.marks.superscript) text.push(item("superscript", "Superscript", "toggle_superscript"));
  if (text.length > 0) groups.push({ id: "text", label: "Text", items: text, advanced: false });

  const lists: EditorToolbarItem[] = [];
  if (capabilities.blocks.bulletList) lists.push(item("bullets", "Bullet list", "toggle_bullet_list"));
  if (capabilities.blocks.numberedList) lists.push(item("numbers", "Numbered list", "toggle_numbered_list"));
  if (capabilities.checklists.ordinary) lists.push(item("tasks", "Task list", "toggle_task_list"));
  if (capabilities.checklists.answerKey) {
    lists.push(item("answer-checklist", "Answer checklist", "insert_answer_checklist"));
  }
  if (lists.length > 0) groups.push({ id: "lists", label: "Lists", items: lists, advanced: false });

  const insert: EditorToolbarItem[] = [];
  if (capabilities.blocks.horizontalRule) {
    insert.push(item("divider", "Horizontal divider", "insert_horizontal_rule"));
  }
  if (capabilities.tables.insert) insert.push(item("table", "Insert table", "insert_table"));
  if (insert.length > 0) groups.push({ id: "insert", label: "Insert", items: insert, advanced: true });

  if (capabilities.tables.rows || capabilities.tables.columns || capabilities.tables.headerRow) {
    const table: EditorToolbarItem[] = [];
    if (capabilities.tables.rows) {
      table.push(item("add-row", "Add row", "add_table_row"));
      table.push(item("delete-row", "Delete row", "delete_table_row"));
    }
    if (capabilities.tables.columns) {
      table.push(item("add-column", "Add column", "add_table_column"));
      table.push(item("delete-column", "Delete column", "delete_table_column"));
    }
    if (capabilities.tables.headerRow) {
      table.push(item("header-row", "Toggle header row", "toggle_table_header_row"));
    }
    table.push(item("delete-table", "Delete table", "delete_table"));
    groups.push({ id: "table", label: "Table", items: table, advanced: true });
  }

  groups.push({
    id: "history",
    label: "History and advanced formatting",
    advanced: true,
    items: [
      item("undo", "Undo", "undo"),
      item("redo", "Redo", "redo"),
      item("strike", "Strikethrough", "toggle_strike"),
      item("inline-code", "Inline code", "toggle_inline_code"),
      item("quote", "Block quote", "toggle_blockquote"),
      item("code-block", "Code block", "toggle_code_block"),
    ],
  });
  return groups;
}

function item(id: string, label: string, command: EditorCommand): EditorToolbarItem {
  return { id, label, command };
}

import type { RichDocument } from "../ipc/common";

/**
 * Editor-neutral feature names used by toolbar, keyboard, and smoke tests.
 * No Tiptap or ProseMirror type crosses this boundary.
 */
export type EditorFeature =
  | "paragraph"
  | "heading"
  | "bold"
  | "italic"
  | "underline"
  | "strike"
  | "inline-code"
  | "subscript"
  | "superscript"
  | "bullet-list"
  | "ordered-list"
  | "blockquote"
  | "code-block"
  | "task-list"
  | "answer-key-checklist"
  | "table"
  | "link"
  | "image"
  | "text-color"
  | "highlight"
  | "text-align"
  | "indent"
  | "undo"
  | "redo";

export interface EditorCapabilities {
  readonly schemaVersion: number;
  readonly features: ReadonlySet<EditorFeature>;
  readonly table: {
    readonly insert: boolean;
    readonly rowsAndColumns: boolean;
    readonly headerRow: boolean;
    readonly mergeAndSplit: boolean;
  };
  readonly checklist: {
    readonly ordinary: boolean;
    readonly answerKey: boolean;
    readonly stableItemIds: boolean;
  };
}

export type EditorBlock = "paragraph" | "heading-1" | "heading-2" | "heading-3" | "heading-4";
export type EditorMark =
  | "bold"
  | "italic"
  | "underline"
  | "strike"
  | "inline-code"
  | "subscript"
  | "superscript";

export type EditorCommand =
  | { type: "set-block"; block: EditorBlock }
  | { type: "toggle-mark"; mark: EditorMark }
  | { type: "toggle-bullet-list" }
  | { type: "toggle-ordered-list" }
  | { type: "toggle-blockquote" }
  | { type: "toggle-code-block" }
  | { type: "set-text-align"; alignment: "left" | "center" | "right" }
  | { type: "change-indent"; delta: -1 | 1 }
  | { type: "set-font-family"; value: string | null }
  | { type: "set-font-size"; value: string | null }
  | { type: "set-text-color"; value: string | null }
  | { type: "set-highlight"; value: string | null }
  | { type: "set-link"; href: string | null }
  | { type: "insert-text"; value: string }
  | { type: "insert-horizontal-rule" }
  | { type: "insert-image"; attrs: Record<string, unknown> }
  | { type: "insert-table"; rows: number; columns: number; headerRow: boolean }
  | { type: "add-row-before" }
  | { type: "add-row-after" }
  | { type: "delete-row" }
  | { type: "add-column-before" }
  | { type: "add-column-after" }
  | { type: "delete-column" }
  | { type: "toggle-header-row" }
  | { type: "merge-cells" }
  | { type: "split-cell" }
  | { type: "delete-table" }
  | { type: "toggle-task-list"; mode: "ordinary" | "answer_key" }
  | { type: "toggle-task-checked" }
  | { type: "toggle-task-correct" }
  | { type: "undo" }
  | { type: "redo" }
  | { type: "clear-formatting" };

export interface EditorSelectionState {
  readonly block: EditorBlock;
  readonly activeMarks: ReadonlySet<EditorMark>;
  readonly bulletList: boolean;
  readonly orderedList: boolean;
  readonly blockquote: boolean;
  readonly codeBlock: boolean;
  readonly textAlign: "left" | "center" | "right";
  readonly fontFamily: string | null;
  readonly fontSize: string | null;
  readonly linkHref: string | null;
  readonly inTable: boolean;
  readonly inTaskList: boolean;
  readonly taskListMode: "ordinary" | "answer_key" | null;
  readonly taskChecked: boolean;
  readonly taskCorrect: boolean;
  readonly canUndo: boolean;
  readonly canRedo: boolean;
}

export interface EditorChangeEvent {
  readonly document: RichDocument;
  readonly plainText: string;
}

export interface EditorAdapter {
  readonly capabilities: EditorCapabilities;
  readonly selection: EditorSelectionState;
  getDocument(): RichDocument;
  getPlainText(): string;
  isActive(feature: EditorFeature): boolean;
  canExecute(command: EditorCommand): boolean;
  execute(command: EditorCommand): boolean;
  focus(): void;
}

export const RICH_DOCUMENT_SCHEMA_VERSION = 2;

export const DEFAULT_EDITOR_CAPABILITIES: EditorCapabilities = Object.freeze({
  schemaVersion: RICH_DOCUMENT_SCHEMA_VERSION,
  features: new Set<EditorFeature>([
    "paragraph",
    "heading",
    "bold",
    "italic",
    "underline",
    "strike",
    "inline-code",
    "subscript",
    "superscript",
    "bullet-list",
    "ordered-list",
    "blockquote",
    "code-block",
    "task-list",
    "answer-key-checklist",
    "table",
    "link",
    "image",
    "text-color",
    "highlight",
    "text-align",
    "indent",
    "undo",
    "redo",
  ]),
  table: Object.freeze({
    insert: true,
    rowsAndColumns: true,
    headerRow: true,
    mergeAndSplit: true,
  }),
  checklist: Object.freeze({
    ordinary: true,
    answerKey: true,
    stableItemIds: true,
  }),
});

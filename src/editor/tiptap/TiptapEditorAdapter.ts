import type { Editor } from "@tiptap/core";
import {
  addColumnAfter,
  addColumnBefore,
  addRowAfter,
  addRowBefore,
  deleteColumn,
  deleteRow,
  deleteTable,
  mergeCells,
  splitCell,
  toggleHeaderRow,
  type Command as ProseMirrorCommand,
} from "@tiptap/pm/tables";

import type {
  EditorAdapter,
  EditorBlock,
  EditorCommand,
  EditorFeature,
  EditorMark,
  EditorSelectionState,
} from "../EditorAdapter";
import { DEFAULT_EDITOR_CAPABILITIES } from "../EditorAdapter";
import { canonicalizeDocument } from "../richText";

const MARK_NAMES: Record<EditorMark, string> = {
  bold: "bold",
  italic: "italic",
  underline: "underline",
  strike: "strike",
  "inline-code": "code",
  subscript: "subscript",
  superscript: "superscript",
};

export class TiptapEditorAdapter implements EditorAdapter {
  readonly capabilities = DEFAULT_EDITOR_CAPABILITIES;

  constructor(private readonly editor: Editor) {}

  get selection(): EditorSelectionState {
    const textStyle = this.editor.getAttributes("textStyle") as Record<string, unknown>;
    const taskList = this.editor.getAttributes("taskList") as Record<string, unknown>;
    const taskItem = this.editor.getAttributes("taskItem") as Record<string, unknown>;
    return {
      block: activeBlock(this.editor),
      activeMarks: new Set(
        (Object.keys(MARK_NAMES) as EditorMark[]).filter((mark) =>
          this.editor.isActive(MARK_NAMES[mark]),
        ),
      ),
      bulletList: this.editor.isActive("bulletList"),
      orderedList: this.editor.isActive("orderedList"),
      blockquote: this.editor.isActive("blockquote"),
      codeBlock: this.editor.isActive("codeBlock"),
      textAlign: activeTextAlignment(this.editor),
      fontFamily: stringAttribute(textStyle.fontFamily),
      fontSize: stringAttribute(textStyle.fontSize),
      linkHref: stringAttribute(this.editor.getAttributes("link").href),
      inTable: this.editor.isActive("table"),
      inTaskList: this.editor.isActive("taskList"),
      taskListMode:
        taskList.mode === "answer_key"
          ? "answer_key"
          : this.editor.isActive("taskList")
            ? "ordinary"
            : null,
      taskChecked: taskItem.checked === true,
      taskCorrect: taskItem.correct === true,
      canUndo: this.editor.can().chain().focus().undo().run(),
      canRedo: this.editor.can().chain().focus().redo().run(),
    };
  }

  getDocument() {
    return canonicalizeDocument(this.editor.getJSON());
  }

  getPlainText(): string {
    return this.editor.getText({ blockSeparator: "\n" });
  }

  isActive(feature: EditorFeature): boolean {
    const selection = this.selection;
    if (feature in MARK_NAMES) return selection.activeMarks.has(feature as EditorMark);
    switch (feature) {
      case "paragraph":
        return selection.block === "paragraph";
      case "heading":
        return selection.block.startsWith("heading-");
      case "bullet-list":
        return selection.bulletList;
      case "ordered-list":
        return selection.orderedList;
      case "blockquote":
        return selection.blockquote;
      case "code-block":
        return selection.codeBlock;
      case "task-list":
        return selection.inTaskList && selection.taskListMode === "ordinary";
      case "answer-key-checklist":
        return selection.inTaskList && selection.taskListMode === "answer_key";
      case "table":
        return selection.inTable;
      case "link":
        return selection.linkHref !== null;
      default:
        return false;
    }
  }

  canExecute(command: EditorCommand): boolean {
    switch (command.type) {
      case "undo":
        return this.selection.canUndo;
      case "redo":
        return this.selection.canRedo;
      case "add-row-before":
        return canRun(this.editor, addRowBefore);
      case "add-row-after":
        return canRun(this.editor, addRowAfter);
      case "delete-row":
        return canRun(this.editor, deleteRow);
      case "add-column-before":
        return canRun(this.editor, addColumnBefore);
      case "add-column-after":
        return canRun(this.editor, addColumnAfter);
      case "delete-column":
        return canRun(this.editor, deleteColumn);
      case "toggle-header-row":
        return canRun(this.editor, toggleHeaderRow);
      case "merge-cells":
        return canRun(this.editor, mergeCells);
      case "split-cell":
        return canRun(this.editor, splitCell);
      case "delete-table":
        return canRun(this.editor, deleteTable);
      default:
        return this.editor.isEditable;
    }
  }

  execute(command: EditorCommand): boolean {
    if (!this.canExecute(command)) return false;
    const chain = this.editor.chain().focus();
    switch (command.type) {
      case "set-block":
        return setBlock(this.editor, command.block);
      case "toggle-mark": {
        const mark = MARK_NAMES[command.mark];
        if (command.mark === "subscript") chain.unsetMark("superscript");
        if (command.mark === "superscript") chain.unsetMark("subscript");
        return chain.toggleMark(mark).run();
      }
      case "toggle-bullet-list":
        return chain.toggleBulletList().run();
      case "toggle-ordered-list":
        return chain.toggleOrderedList().run();
      case "toggle-blockquote":
        return chain.toggleBlockquote().run();
      case "toggle-code-block":
        return chain.toggleCodeBlock().run();
      case "set-text-align":
        return chain.setTextAlign(command.alignment).run();
      case "change-indent":
        return changeIndent(this.editor, command.delta);
      case "set-font-family":
        return command.value
          ? chain.setFontFamily(command.value).run()
          : chain.unsetFontFamily().run();
      case "set-font-size":
        return command.value
          ? chain.setFontSize(command.value).run()
          : chain.unsetFontSize().run();
      case "set-text-color":
        return command.value ? chain.setColor(command.value).run() : chain.unsetColor().run();
      case "set-highlight":
        return command.value
          ? chain.setHighlight({ color: command.value }).run()
          : chain.unsetHighlight().run();
      case "set-link":
        return command.href
          ? chain.extendMarkRange("link").setLink({ href: command.href }).run()
          : chain.extendMarkRange("link").unsetLink().run();
      case "insert-text":
        return chain.insertContent(command.value).run();
      case "insert-horizontal-rule":
        return chain.setHorizontalRule().run();
      case "insert-image":
        return chain.setImage(command.attrs as never).run();
      case "insert-table":
        return chain.insertContent(tableDocument(command.rows, command.columns, command.headerRow)).run();
      case "add-row-before":
        return run(this.editor, addRowBefore);
      case "add-row-after":
        return run(this.editor, addRowAfter);
      case "delete-row":
        return run(this.editor, deleteRow);
      case "add-column-before":
        return run(this.editor, addColumnBefore);
      case "add-column-after":
        return run(this.editor, addColumnAfter);
      case "delete-column":
        return run(this.editor, deleteColumn);
      case "toggle-header-row":
        return run(this.editor, toggleHeaderRow);
      case "merge-cells":
        return run(this.editor, mergeCells);
      case "split-cell":
        return run(this.editor, splitCell);
      case "delete-table":
        return run(this.editor, deleteTable);
      case "toggle-task-list": {
        if (this.editor.isActive("taskList")) {
          if (this.selection.taskListMode !== command.mode) {
            return chain.updateAttributes("taskList", { mode: command.mode }).run();
          }
          return chain.toggleList("taskList", "taskItem").run();
        }
        return chain
          .toggleList("taskList", "taskItem")
          .updateAttributes("taskList", { mode: command.mode })
          .run();
      }
      case "toggle-task-checked":
        return chain.updateAttributes("taskItem", { checked: !this.selection.taskChecked }).run();
      case "toggle-task-correct":
        return chain.updateAttributes("taskItem", { correct: !this.selection.taskCorrect }).run();
      case "undo":
        return chain.undo().run();
      case "redo":
        return chain.redo().run();
      case "clear-formatting":
        return chain.unsetAllMarks().clearNodes().run();
    }
  }

  focus(): void {
    this.editor.commands.focus();
  }
}

function activeBlock(editor: Editor): EditorBlock {
  for (const level of [1, 2, 3, 4] as const) {
    if (editor.isActive("heading", { level })) return `heading-${level}`;
  }
  return "paragraph";
}

function activeTextAlignment(editor: Editor): "left" | "center" | "right" {
  if (editor.isActive({ textAlign: "center" })) return "center";
  if (editor.isActive({ textAlign: "right" })) return "right";
  return "left";
}

function setBlock(editor: Editor, block: EditorBlock): boolean {
  if (block === "paragraph") return editor.chain().focus().setParagraph().run();
  const level = Number(block.slice("heading-".length)) as 1 | 2 | 3 | 4;
  return editor.chain().focus().setHeading({ level }).run();
}

function changeIndent(editor: Editor, delta: -1 | 1): boolean {
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

function tableDocument(rows: number, columns: number, headerRow: boolean): Record<string, unknown> {
  const boundedRows = Math.min(20, Math.max(1, Math.trunc(rows)));
  const boundedColumns = Math.min(12, Math.max(1, Math.trunc(columns)));
  return {
    type: "table",
    content: Array.from({ length: boundedRows }, (_, rowIndex) => ({
      type: "tableRow",
      content: Array.from({ length: boundedColumns }, () => ({
        type: headerRow && rowIndex === 0 ? "tableHeader" : "tableCell",
        content: [{ type: "paragraph" }],
      })),
    })),
  };
}

function stringAttribute(value: unknown): string | null {
  return typeof value === "string" && value !== "" ? value : null;
}

function canRun(editor: Editor, command: ProseMirrorCommand): boolean {
  return command(editor.state);
}

function run(editor: Editor, command: ProseMirrorCommand): boolean {
  return command(editor.state, editor.view.dispatch, editor.view);
}

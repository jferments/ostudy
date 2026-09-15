import { Editor } from "@tiptap/core";
import { afterEach, describe, expect, it } from "vitest";

import { createEditorExtensions } from "./extensions";

let editor: Editor | null = null;

afterEach(() => {
  editor?.destroy();
  editor = null;
});

function createCheckboxEditor(text = "dogs"): Editor {
  editor = new Editor({
    element: document.createElement("div"),
    extensions: createEditorExtensions({ templatePlaceholders: false }),
    content: {
      type: "doc",
      attrs: { schemaVersion: 3 },
      content: [
        {
          type: "taskList",
          attrs: { mode: "ordinary" },
          content: [
            {
              type: "taskItem",
              attrs: { checked: false, itemId: null },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text }],
                },
              ],
            },
          ],
        },
      ],
    },
  });
  setCursorAfterText(editor, text);
  return editor;
}

function setCursorAfterText(current: Editor, text: string): void {
  let cursor: number | null = null;
  current.state.doc.descendants((node, pos) => {
    if (cursor === null && node.isText && node.text === text) {
      cursor = pos + node.nodeSize;
      return false;
    }
    return true;
  });
  if (cursor === null) throw new Error(`Could not find text node: ${text}`);
  current.commands.setTextSelection(cursor);
}

describe("ordinary checkbox list editing", () => {
  it("creates a sibling checkbox when Enter is pressed and keeps later text in that item", () => {
    const current = createCheckboxEditor();

    expect(current.commands.keyboardShortcut("Enter")).toBe(true);
    expect(current.commands.insertContent("cats")).toBe(true);

    const list = current.state.doc.child(0);
    expect(list.type.name).toBe("taskList");
    expect(list.attrs).toMatchObject({ mode: "ordinary" });
    expect(list.childCount).toBe(2);
    expect(list.child(0).attrs).toMatchObject({ checked: false, itemId: null });
    expect(list.child(1).attrs).toMatchObject({ checked: false, itemId: null });
    expect(list.child(0).textContent).toBe("dogs");
    expect(list.child(1).textContent).toBe("cats");
  });

  it("exits the checkbox list when Enter is pressed again on an empty checkbox", () => {
    const current = createCheckboxEditor();

    expect(current.commands.keyboardShortcut("Enter")).toBe(true);
    expect(current.commands.keyboardShortcut("Enter")).toBe(true);

    const list = current.state.doc.child(0);
    expect(list.type.name).toBe("taskList");
    expect(list.childCount).toBe(1);
    expect(list.child(0).textContent).toBe("dogs");

    const { $from } = current.state.selection;
    const ancestors = Array.from(
      { length: $from.depth + 1 },
      (_, depth) => $from.node(depth).type.name,
    );
    expect(ancestors).not.toContain("taskList");
    expect($from.parent.type.name).toBe("paragraph");
  });
});

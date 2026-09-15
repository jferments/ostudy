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

function checkboxList(current: Editor) {
  return current.getJSON().content?.find((node) => node.type === "taskList");
}

describe("ordinary checkbox list editing", () => {
  it("creates a sibling checkbox when Enter is pressed and keeps later text in that item", () => {
    const current = createCheckboxEditor();

    expect(current.commands.keyboardShortcut("Enter")).toBe(true);
    expect(current.commands.insertContent("cats")).toBe(true);

    const list = checkboxList(current);
    expect(list?.attrs).toMatchObject({ mode: "ordinary" });
    expect(list?.content).toHaveLength(2);
    expect(list?.content?.map((item) => item.attrs)).toEqual([
      { checked: false, itemId: null },
      { checked: false, itemId: null },
    ]);
    expect(
      list?.content?.map((item) => item.content?.[0]?.content?.[0]?.text),
    ).toEqual(["dogs", "cats"]);
  });

  it("exits the checkbox list when Enter is pressed again on an empty checkbox", () => {
    const current = createCheckboxEditor();

    expect(current.commands.keyboardShortcut("Enter")).toBe(true);
    expect(current.commands.keyboardShortcut("Enter")).toBe(true);

    const list = checkboxList(current);
    expect(list?.content).toHaveLength(1);
    expect(list?.content?.[0]?.content?.[0]?.content?.[0]?.text).toBe("dogs");

    const { $from } = current.state.selection;
    const ancestors = Array.from(
      { length: $from.depth + 1 },
      (_, depth) => $from.node(depth).type.name,
    );
    expect(ancestors).not.toContain("taskList");
    expect($from.parent.type.name).toBe("paragraph");
  });
});

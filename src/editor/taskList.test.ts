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

    expect(current.getJSON()).toMatchObject({
      type: "doc",
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
                  content: [{ type: "text", text: "dogs" }],
                },
              ],
            },
            {
              type: "taskItem",
              attrs: { checked: false, itemId: null },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "cats" }],
                },
              ],
            },
          ],
        },
      ],
    });
  });

  it("exits the checkbox list when Enter is pressed again on an empty checkbox", () => {
    const current = createCheckboxEditor();

    expect(current.commands.keyboardShortcut("Enter")).toBe(true);
    expect(current.commands.keyboardShortcut("Enter")).toBe(true);

    expect(current.getJSON()).toMatchObject({
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "dogs" }],
                },
              ],
            },
          ],
        },
        { type: "paragraph" },
      ],
    });
  });
});

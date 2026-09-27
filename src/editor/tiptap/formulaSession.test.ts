import { Editor, type JSONContent } from "@tiptap/core";
import { undoDepth } from "@tiptap/pm/history";
import { AllSelection, NodeSelection, TextSelection } from "@tiptap/pm/state";
import { afterEach, describe, expect, it, vi } from "vitest";

import { requiredTestValue as required } from "../../test/factories";
import type { RichDocument } from "../../ipc/common";
import { createEditorExtensions } from "../extensions";
import type { FormulaPlacement } from "../formula";
import { createTiptapEditorAdapter } from "../tiptapAdapter";

vi.mock("../../platform/assets", () => ({ managedAssetUrl: () => "" }));

const editors: Editor[] = [];
afterEach(() => {
  for (const editor of editors.splice(0)) editor.destroy();
});

function open(content: JSONContent[] = [{ type: "paragraph", content: [{ type: "text", text: "left right" }] }]) {
  const editor = new Editor({
    element: document.createElement("div"),
    extensions: createEditorExtensions(),
    content: { type: "doc", attrs: { schemaVersion: 2 }, content },
  });
  editors.push(editor);
  return editor;
}

function select(editor: Editor, from: number, to = from) {
  editor.view.dispatch(editor.state.tr.setSelection(TextSelection.create(editor.state.doc, from, to)));
}

function begin(editor: Editor) {
  const session = createTiptapEditorAdapter(editor).beginFormulaEdit();
  expect(session).not.toBeNull();
  return session!;
}

function formulas(editor: Editor) {
  const result: unknown[] = [];
  editor.state.doc.descendants((node) => {
    if (["inlineMath", "blockMath"].includes(node.type.name)) result.push(node.toJSON());
  });
  return result;
}

function selectedMath(editor: Editor, name = "inlineMath") {
  let position: number | undefined;
  editor.state.doc.descendants((node, pos) => {
    if (node.type.name === name && position === undefined) position = pos;
  });
  expect(position).not.toBeUndefined();
  editor.view.dispatch(editor.state.tr.setSelection(NodeSelection.create(editor.state.doc, position!)));
}

describe("captured formula editing through the real adapter", () => {
  it("saves at the original selection, preserves exact source, and round-trips schema three", () => {
    const editor = open();
    select(editor, 6);
    const session = begin(editor);
    expect(session.initialValue).toEqual({ latex: "", placement: "inline" });
    select(editor, 1); // A dialog/toolbar changed the live editor selection.
    expect(session.save({ latex: " \\frac{a}{b}\n", placement: "inline" })).toEqual({ ok: true });
    expect(createTiptapEditorAdapter(editor).plainText()).toBe("left \\( \\frac{a}{b}\n\\)right");
    const saved = createTiptapEditorAdapter(editor).document();
    expect(saved.attrs).toEqual({ schemaVersion: 3 });
    const reopened = open(saved.content as JSONContent[]);
    expect(formulas(reopened)).toEqual(formulas(editor));
    expect(session.save({ latex: "y", placement: "inline" })).toMatchObject({ reason: "unavailable" });
  });

  it("cancels without a content mutation or undo event and restores the original selection", () => {
    const editor = open();
    select(editor, 2, 5);
    const before = editor.state.doc;
    const depth = undoDepth(editor.state);
    const session = begin(editor);
    select(editor, 9);
    session.cancel();
    session.cancel();
    expect(editor.state.doc).toBe(before);
    expect(editor.state.selection.from).toBe(2);
    expect(editor.state.selection.to).toBe(5);
    expect(undoDepth(editor.state)).toBe(depth);
    expect(session.save({ latex: "x", placement: "inline" })).toMatchObject({ reason: "unavailable" });
  });

  it("allows correction after invalid drafts without a partial write or history entry", () => {
    const editor = open();
    const before = editor.state.doc;
    const session = begin(editor);
    for (const latex of [" ", "x\0y", "α".repeat(8193)]) {
      expect(session.save({ latex, placement: "inline" })).toMatchObject({ reason: "invalid-source" });
      expect(editor.state.doc).toBe(before);
    }
    expect(session.save({ latex: "x", placement: "bad" as FormulaPlacement }))
      .toMatchObject({ reason: "invalid-placement" });
    expect(undoDepth(editor.state)).toBe(0);
    expect(session.save({ latex: "x", placement: "inline" })).toEqual({ ok: true });
    expect(undoDepth(editor.state)).toBe(1);
  });

  it("isolates a single formula save from both prior and subsequent typing", () => {
    const editor = open();
    editor.view.dispatch(editor.state.tr.insertText("A", 1));
    select(editor, 2);
    const before = editor.getJSON();
    const session = begin(editor);
    expect(session.save({ latex: "x", placement: "inline" })).toEqual({ ok: true });
    const withFormula = editor.getJSON();
    editor.view.dispatch(editor.state.tr.insertText("B", 3));
    expect(undoDepth(editor.state)).toBe(3);
    expect(editor.commands.undo()).toBe(true);
    expect(editor.getJSON()).toEqual(withFormula);
    expect(editor.commands.undo()).toBe(true);
    expect(editor.getJSON()).toEqual(before);
    expect(editor.commands.redo()).toBe(true);
    expect(editor.getJSON()).toEqual(withFormula);
  });

  it("edits a selected formula without inheriting marks or replacing neighbouring text", () => {
    const editor = open([{ type: "paragraph", content: [
      { type: "text", text: "left ", marks: [{ type: "bold" }] },
      { type: "inlineMath", attrs: { latex: "old" } },
      { type: "text", text: " right" },
    ] }]);
    selectedMath(editor);
    const before = editor.getJSON();
    const session = begin(editor);
    expect(session.initialValue).toEqual({ latex: "old", placement: "inline" });
    expect(session.save({ latex: "new", placement: "inline" })).toEqual({ ok: true });
    expect(formulas(editor)).toEqual([{ type: "inlineMath", attrs: { latex: "new" } }]);
    expect(editor.getHTML()).toContain("<strong>left </strong>");
    expect(editor.getText()).toContain(" right");
    expect(editor.commands.undo()).toBe(true);
    expect(editor.getJSON()).toEqual(before);
  });

  it("does not create a history step when an existing formula is saved unchanged", () => {
    const editor = open([{ type: "blockMath", attrs: { latex: "x" } }]);
    selectedMath(editor, "blockMath");
    const before = editor.state.doc;
    // Initial editor normalization may already have a history entry. The save
    // must preserve existing history, not assume or reset it to an empty stack.
    const historyBefore = undoDepth(editor.state);
    const session = begin(editor);
    expect(session.save({ ...session.initialValue })).toEqual({ ok: true });
    expect(editor.state.doc).toBe(before);
    expect(undoDepth(editor.state)).toBe(historyBefore);
  });

  it("inserts a block in text without dropping either side, and can convert it to inline", () => {
    const editor = open();
    select(editor, 6);
    const before = editor.getJSON();
    expect(begin(editor).save({ latex: "x", placement: "block" })).toEqual({ ok: true });
    expect(editor.state.doc.childCount).toBe(3);
    expect(editor.state.doc.child(0).textContent).toBe("left ");
    expect(editor.state.doc.child(1).type.name).toBe("blockMath");
    expect(editor.state.doc.child(2).textContent).toBe("right");
    selectedMath(editor, "blockMath");
    expect(begin(editor).save({ latex: "y", placement: "inline" })).toEqual({ ok: true });
    expect(editor.state.doc.child(1).type.name).toBe("paragraph");
    expect(formulas(editor)).toEqual([{ type: "inlineMath", attrs: { latex: "y" } }]);
    expect(editor.commands.undo()).toBe(true);
    expect(editor.state.doc.child(1).type.name).toBe("blockMath");
    expect(editor.commands.undo()).toBe(true);
    expect(editor.getJSON()).toEqual(before);
  });

  it("preserves quote, list and table containers when changing block placement", () => {
    for (const content of [
      [{ type: "blockquote", content: [{ type: "blockMath", attrs: { latex: "x" } }] }],
      [{ type: "bulletList", content: [{ type: "listItem", content: [
        { type: "paragraph", content: [{ type: "text", text: "list text" }] },
        { type: "blockMath", attrs: { latex: "x" } },
      ] }] }],
      [{ type: "table", content: [{ type: "tableRow", content: [{ type: "tableCell", content: [
        { type: "blockMath", attrs: { latex: "x" } },
      ] }] }] }],
    ]) {
      const editor = open(content);
      selectedMath(editor, "blockMath");
      const before = editor.getJSON();
      expect(begin(editor).save({ latex: "y", placement: "inline" })).toEqual({ ok: true });
      expect(editor.state.doc.firstChild?.type.name).toBe(required(content[0], "container fixture").type);
      expect(formulas(editor)).toEqual([{ type: "inlineMath", attrs: { latex: "y" } }]);
      expect(() => editor.state.doc.check()).not.toThrow();
      expect(editor.commands.undo()).toBe(true);
      expect(editor.getJSON()).toEqual(before);
    }
  });

  it("replaces only the captured text range and does not inherit bold or link marks", () => {
    const editor = open([{ type: "paragraph", content: [{ type: "text", text: "left right", marks: [
      { type: "bold" }, { type: "link", attrs: { href: "https://example.invalid" } },
    ] }] }]);
    select(editor, 1, 5);
    expect(begin(editor).save({ latex: "x", placement: "inline" })).toEqual({ ok: true });
    expect(formulas(editor)).toEqual([{ type: "inlineMath", attrs: { latex: "x" } }]);
    expect(createTiptapEditorAdapter(editor).plainText()).toBe("\\(x\\) right");
  });

  it("refuses stale content even if a replacement has the same length or equal JSON", () => {
    for (const equal of [false, true]) {
      const editor = open();
      select(editor, 6);
      const session = begin(editor);
      const replacement = equal ? editor.getJSON() : { type: "doc", content: [
        { type: "paragraph", content: [{ type: "text", text: "other text" }] },
      ] };
      createTiptapEditorAdapter(editor).setDocument(replacement as RichDocument);
      select(editor, 2);
      const afterReplacement = editor.state.doc;
      expect(session.save({ latex: "x", placement: "inline" })).toMatchObject({ reason: "stale-document" });
      session.cancel();
      expect(editor.state.doc).toBe(afterReplacement);
      expect(editor.state.selection.from).toBe(2);
      expect(formulas(editor)).toEqual([]);
    }
  });

  it("does not write after the editor becomes read-only or is destroyed", () => {
    const editor = open();
    const session = begin(editor);
    const before = editor.state.doc;
    editor.setEditable(false);
    expect(session.save({ latex: "x", placement: "inline" })).toMatchObject({ reason: "unavailable" });
    session.cancel();
    expect(editor.state.doc).toBe(before);
    expect(createTiptapEditorAdapter(editor).beginFormulaEdit()).toBeNull();
    const destroyed = open();
    const destroyedSession = begin(destroyed);
    destroyed.destroy();
    expect(destroyedSession.save({ latex: "x", placement: "inline" })).toMatchObject({ reason: "unavailable" });
    expect(() => destroyedSession.cancel()).not.toThrow();
    expect(createTiptapEditorAdapter(destroyed).beginFormulaEdit()).toBeNull();
  });

  it("rejects code, multi-block and whole-document selections before opening a dialog", () => {
    const code = open([{ type: "codeBlock", content: [{ type: "text", text: "literal" }] }]);
    select(code, 2);
    expect(createTiptapEditorAdapter(code).beginFormulaEdit()).toBeNull();
    const editor = open([
      { type: "paragraph", content: [{ type: "text", text: "one" }] },
      { type: "paragraph", content: [{ type: "text", text: "two" }] },
    ]);
    select(editor, 2, 7);
    expect(createTiptapEditorAdapter(editor).beginFormulaEdit()).toBeNull();
    editor.view.dispatch(editor.state.tr.setSelection(new AllSelection(editor.state.doc)));
    expect(createTiptapEditorAdapter(editor).beginFormulaEdit()).toBeNull();
  });
});

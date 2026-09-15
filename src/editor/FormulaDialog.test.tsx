import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { Editor, type JSONContent } from "@tiptap/core";
import { undoDepth } from "@tiptap/pm/history";
import { NodeSelection, TextSelection } from "@tiptap/pm/state";
import { StrictMode, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mountFormulaInput } from "../math/mathlive";
import { LayoutModeProvider } from "../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { createEditorExtensions } from "./extensions";
import { createTiptapEditorAdapter } from "./tiptapAdapter";
import type { FormulaEditSession } from "./formula";
import { FormulaDialog } from "./FormulaDialog";
import { RichTextEditor } from "./RichTextEditor";

vi.mock("../math/mathlive", () => ({ mountFormulaInput: vi.fn() }));
const editors: Editor[] = [];
const originalMatchMedia = window.matchMedia;
const hideKeyboard = vi.fn(() => false);
const showKeyboard = vi.fn(() => true);
const dispose = vi.fn();
const toggleKeyboard = vi.fn(() => true);
const applyTemplate = vi.fn(() => true);
beforeEach(() => {
  window.matchMedia = vi.fn().mockImplementation((media: string) => ({ matches: false, media, addEventListener: vi.fn(), removeEventListener: vi.fn(), addListener: vi.fn(), removeListener: vi.fn(), dispatchEvent: vi.fn() }));
  hideKeyboard.mockReturnValue(false);
  showKeyboard.mockReturnValue(true);
  toggleKeyboard.mockReturnValue(true);
  applyTemplate.mockReturnValue(true);
  vi.mocked(mountFormulaInput).mockImplementation(async () => ({ focus: vi.fn(), applyTemplate, dispose, showKeyboard, toggleKeyboard, hideKeyboard }));
});
afterEach(() => {
  editors.splice(0).forEach((editor) => editor.destroy());
  window.matchMedia = originalMatchMedia;
});
function open(content: JSONContent[] = [{ type: "paragraph", content: [{ type: "text", text: "left right" }] }]) {
  const editor = new Editor({ element: document.createElement("div"), extensions: createEditorExtensions(), content: { type: "doc", content } });
  editors.push(editor); return editor;
}
function show(session: FormulaEditSession, strict = false) {
  function Host() {
    const [visible, setVisible] = useState(true);
    return <LayoutModeProvider><OverlayProvider>{visible ? <FormulaDialog session={session} onClose={() => setVisible(false)} /> : null}</OverlayProvider></LayoutModeProvider>;
  }
  return render(strict ? <StrictMode><Host /></StrictMode> : <Host />);
}
async function loaded() {
  await waitFor(() => expect((screen.getByRole("button", { name: "Show math keyboard" }) as HTMLButtonElement).disabled).toBe(false));
}
function type(source: string) {
  act(() => vi.mocked(mountFormulaInput).mock.calls.at(-1)![0].onChange(source));
}

describe("formula dialog with actual editor transactions", () => {
  it("saves at the captured selection with one undo event and a durable source-only round trip", async () => {
    const editor = open(); const adapter = createTiptapEditorAdapter(editor);
    editor.view.dispatch(editor.state.tr.setSelection(TextSelection.create(editor.state.doc, 6)));
    show(adapter.beginFormulaEdit()!); await loaded();
    editor.view.dispatch(editor.state.tr.setSelection(TextSelection.create(editor.state.doc, 1)));
    type(String.raw`\frac{a}{b}`);
    const before = undoDepth(editor.state);
    fireEvent.click(screen.getByRole("button", { name: "Save formula" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(adapter.plainText()).toBe(String.raw`left \(\frac{a}{b}\)right`);
    expect(undoDepth(editor.state)).toBe(before + 1);
    const saved = adapter.document();
    const reopened = open(saved.content as JSONContent[]);
    expect(createTiptapEditorAdapter(reopened).document()).toEqual(saved);
    editor.commands.undo(); expect(adapter.plainText()).toBe("left right");
    editor.commands.redo(); expect(adapter.document()).toEqual(saved);
  });

  it("routes visual structure controls through the captured MathLive input", async () => {
    const editor = open();
    show(createTiptapEditorAdapter(editor).beginFormulaEdit()!); await loaded();
    fireEvent.click(screen.getByRole("button", { name: "Fraction" }));
    expect(applyTemplate).toHaveBeenCalledWith(String.raw`\frac{#@}{#?}`);
    fireEvent.click(screen.getByRole("tab", { name: "Greek" }));
    fireEvent.click(screen.getByRole("button", { name: "Theta" }));
    expect(applyTemplate).toHaveBeenLastCalledWith(String.raw`\theta`);
    expect(editor.state.doc.textContent).toBe("left right");
  });

  it("does not save or consume a session during StrictMode cleanup, cancel, or unchanged editing", async () => {
    const editor = open([{ type: "blockMath", attrs: { latex: " x + 1 " } }]);
    editor.view.dispatch(editor.state.tr.setSelection(NodeSelection.create(editor.state.doc, 0)));
    const adapter = createTiptapEditorAdapter(editor);
    const before = editor.state.doc; const depth = undoDepth(editor.state);
    const first = show(adapter.beginFormulaEdit()!, true); await loaded();
    fireEvent.click(screen.getByRole("button", { name: "Save formula" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(editor.state.doc).toBe(before); expect(undoDepth(editor.state)).toBe(depth);
    first.unmount();
    show(adapter.beginFormulaEdit()!); await loaded(); type("changed");
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(editor.state.doc).toBe(before); expect(undoDepth(editor.state)).toBe(depth);
  });

  it("keeps rejected stale or unsupported drafts from mutating a card", async () => {
    const editor = open(); const adapter = createTiptapEditorAdapter(editor);
    show(adapter.beginFormulaEdit()!); await loaded();
    type(String.raw`\href{https://example.invalid}{x}`);
    fireEvent.click(screen.getByRole("button", { name: "Save formula" }));
    expect(screen.getByRole("alert").textContent).toContain("unsupported");
    type("x");
    editor.commands.setContent("<p>Replacement</p>"); const replacement = editor.state.doc;
    fireEvent.click(screen.getByRole("button", { name: "Save formula" }));
    expect(screen.getByRole("alert").textContent).toContain("content changed");
    expect(editor.state.doc).toBe(replacement);
  });

  it("uses the shared Escape dismissal to hide the keyboard first, then cancel", async () => {
    const editor = open(); const before = editor.state.doc;
    show(createTiptapEditorAdapter(editor).beginFormulaEdit()!); await loaded();
    hideKeyboard.mockReturnValueOnce(true);
    const dialog = screen.getByRole("dialog", { name: "Edit formula" });
    fireEvent.click(screen.getByRole("button", { name: "Show math keyboard" }));
    expect(toggleKeyboard).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "Hide math keyboard" })).toBeTruthy();
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(screen.getByRole("dialog")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Show math keyboard" })).toBeTruthy();
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(editor.state.doc).toBe(before);
  });

  it("aborts a pending load on dismissal and never focuses its late result", async () => {
    let resolve!: (input: Awaited<ReturnType<typeof mountFormulaInput>>) => void;
    vi.mocked(mountFormulaInput).mockReturnValue(new Promise((done) => { resolve = done; }));
    show(createTiptapEditorAdapter(open()).beginFormulaEdit()!);
    // Wait for the shared portal to actually mount, not an arbitrary timeout.
    await waitFor(() => expect(mountFormulaInput).toHaveBeenCalled());
    const signal = vi.mocked(mountFormulaInput).mock.calls.at(-1)![0].signal;
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(signal.aborted).toBe(true));
    const focus = vi.fn(); const release = vi.fn();
    await act(async () => resolve({ focus, applyTemplate, dispose: release, showKeyboard, toggleKeyboard, hideKeyboard }));
    expect(focus).not.toHaveBeenCalled(); expect(release).toHaveBeenCalledOnce();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens from the real toolbar and closes without a content write when editing becomes disabled", async () => {
    const props = { label: "Question", assets: [], onChange: vi.fn(), document: { type: "doc" as const, content: [{ type: "paragraph" as const, content: [{ type: "text" as const, text: "Question" }] }] } };
    const view = render(<OverlayProvider><RichTextEditor {...props} /></OverlayProvider>);
    const opener = await screen.findByRole("button", { name: "Insert or edit formula" });
    opener.focus(); fireEvent.mouseDown(opener); fireEvent.click(opener); await loaded();
    view.rerender(<OverlayProvider><RichTextEditor {...props} disabled /></OverlayProvider>);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(props.onChange).not.toHaveBeenCalled();
  });
});

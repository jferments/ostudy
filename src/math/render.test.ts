import { describe, expect, it } from "vitest";
import { Editor } from "@tiptap/core";
import { createEditorExtensions } from "../editor/extensions";
import { createTiptapEditorAdapter } from "../editor/tiptapAdapter";
import { renderFormula } from "./render";
import { formulaRenderingError, formulaSourceError } from "./source";

// These exercise the real pinned KaTeX renderer, not a markup stub.
describe("offline formula rendering", () => {
  it.each([String.raw`\frac{a}{b}`, String.raw`\sqrt{x^2+1}`, String.raw`\begin{pmatrix}a&b\\c&d\end{pmatrix}`])(
    "typesets %s with an accessible MathML representation", (source) => {
      const target = document.createElement("div");
      expect(renderFormula(target, source, true)).toBeNull();
      expect(target.querySelector(".katex-display .katex")).not.toBeNull();
      expect(target.querySelector("math annotation")?.textContent).toBe(source);
      expect(target.querySelector("a,img,iframe,script,style")).toBeNull();
    },
  );

  it.each([String.raw`\href{https://example.invalid}{click}`, String.raw`\includegraphics{https://example.invalid/leak}`,
    String.raw`\htmlData{secret=x}{x}`, String.raw`\gdef\x{\x}\x`, '<img src="https://example.invalid" onerror="alert(1)">',
    String.raw`\unknowncommand{x}`, "x\u0000y", "x".repeat(16385), "{".repeat(65) + "x" + "}".repeat(65)])(
    "shows inert source instead of executing unsupported input %#", (source) => {
      const target = document.createElement("div");
      expect(renderFormula(target, source, false)).not.toBeNull();
      expect(target.dataset.formulaError).toBe("true");
      expect(target.querySelector("a,img,iframe,script,style,math")).toBeNull();
      expect(target.textContent?.startsWith("\\(")).toBe(true);
    },
  );

  it("does not leak errors, titles or macro state into the next formula", () => {
    const target = document.createElement("div");
    renderFormula(target, String.raw`\gdef\secret{42}`, false);
    expect(target.title).not.toBe("");
    expect(renderFormula(target, "x+1", false)).toBeNull();
    expect(target.title).toBe("");
    expect(target.hasAttribute("data-formula-error")).toBe(false);
    expect(target.querySelector(".katex-display")).toBeNull();
    expect(renderFormula(target, String.raw`\secret`, false)).not.toBeNull();
  });

  it("keeps storage admission independent from renderer support and enforces nesting despite unmatched closers", () => {
    expect(formulaSourceError(String.raw`\unsupported{x}`)).toBeNull();
    expect(formulaRenderingError("}".repeat(100) + "{".repeat(65))).not.toBeNull();
  });

  it("typesets actual editor nodes without putting renderer DOM in saved content or clipboard HTML", () => {
    const editor = new Editor({ element: document.createElement("div"), extensions: createEditorExtensions(), content: {
      type: "doc", attrs: { schemaVersion: 3 }, content: [{ type: "paragraph", content: [
        { type: "text", text: "Before " }, { type: "inlineMath", attrs: { latex: String.raw`\frac{a}{b}` } },
        { type: "text", text: " after" },
      ] }],
    } });
    try {
      const adapter = createTiptapEditorAdapter(editor);
      expect(editor.view.dom.querySelector("[data-study-math] .katex")).not.toBeNull();
      expect(editor.getHTML()).not.toContain('class="katex');
      expect(editor.getHTML()).toContain("data-latex");
      editor.view.dispatch(editor.state.tr.insertText("!", 1));
      // The adapter deliberately exposes opaque JSON. Match its actual saved
      // shape rather than indexing unknown content or casting away that boundary.
      expect(adapter.document()).toMatchObject({ content: [{ content: [
        { type: "text", text: "!Before " },
        { type: "inlineMath", attrs: { latex: String.raw`\frac{a}{b}` } },
        { type: "text", text: " after" },
      ] }] });
      expect(JSON.stringify(adapter.document())).not.toContain("MathML");
    } finally { editor.destroy(); }
  });
});

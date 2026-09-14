import { Editor } from "@tiptap/core";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Asset } from "../ipc/assets";
import type { RichDocument } from "../ipc/common";
import { createEditorExtensions } from "./extensions";
import { createTiptapEditorAdapter } from "./tiptapAdapter";
import {
  canonicalizeDocument,
  collectAssetIds,
  extractRichDocumentText,
  hydrateDocument,
  isDocumentEmpty,
} from "./richText";

vi.mock("../platform/assets", () => ({
  managedAssetUrl: (id: string) => `https://ostudy-asset.localhost/${id}`,
}));

const audioId = "a".repeat(32);
const videoId = "b".repeat(32);
const imageId = "c".repeat(32);

function media(type: "audio" | "video" | "image", assetId: string) {
  return { type, attrs: { assetId, src: `study-asset://${assetId}` } };
}

function asset(id: string, mimeType: string): Asset {
  return { id, mimeType, originalFilename: "secret-answer.mp3", byteSize: 100, createdAt: 1 };
}

function doc(content: unknown[], schemaVersion = 3): RichDocument {
  return { type: "doc", attrs: { schemaVersion }, content };
}

describe("math and media rich-document preservation", () => {
  it("retains schema three, formulas, and all managed asset references on save", () => {
    const original = doc([
      { type: "paragraph", content: [{ type: "inlineMath", attrs: { latex: "x^2" } }] },
      { type: "blockMath", attrs: { latex: "\\frac{a}{b}" } },
      media("audio", audioId), media("video", videoId), media("image", imageId),
    ]);
    const hydrated = hydrateDocument(original, [
      asset(audioId, "audio/mpeg"), asset(videoId, "video/mp4"), asset(imageId, "image/png"),
    ]);
    const saved = canonicalizeDocument(hydrated);
    expect(saved.attrs).toEqual({ schemaVersion: 3 });
    expect(collectAssetIds(saved)).toEqual([audioId, videoId, imageId]);
    expect(JSON.stringify(saved)).toContain("\\\\frac{a}{b}");
    expect(JSON.stringify(saved)).not.toContain("ostudy-asset.localhost");
    expect(canonicalizeDocument(saved)).toEqual(saved);
    expect(original.content).toEqual([
      { type: "paragraph", content: [{ type: "inlineMath", attrs: { latex: "x^2" } }] },
      { type: "blockMath", attrs: { latex: "\\frac{a}{b}" } },
      media("audio", audioId), media("video", videoId), media("image", imageId),
    ]);
  });

  it("upgrades newly authored atoms without relabeling future schemas as current", () => {
    const withNewAtom = doc([{ type: "blockMath", attrs: { latex: "x" } }], 2);
    expect(canonicalizeDocument(withNewAtom).attrs).toEqual({ schemaVersion: 3 });
    for (const version of [0, 1.5, 4, Number.NaN]) {
      expect(() => canonicalizeDocument(doc([], version))).toThrow(/version is not supported/u);
    }
  });

  it("never hydrates remote or mismatched media, or reveals an asset filename", () => {
    const original = doc([
      { type: "audio", attrs: { assetId: audioId, src: "https://example.invalid/track" } },
      media("video", videoId),
    ]);
    const hydrated = hydrateDocument(original, [asset(audioId, "audio/mpeg")]);
    expect(hydrated.content).toEqual([
      { type: "audio", attrs: { assetId: audioId, src: `https://ostudy-asset.localhost/${audioId}` } },
      { type: "video", attrs: { assetId: videoId, src: "" } },
    ]);
    expect(JSON.stringify(hydrated)).not.toContain("secret-answer");
    const wrongType = hydrateDocument(original, [asset(audioId, "video/mp4")]);
    expect(JSON.stringify(wrongType)).not.toContain("ostudy-asset.localhost");
    expect(JSON.stringify(wrongType)).not.toContain("example.invalid");
    expect(collectAssetIds(canonicalizeDocument(wrongType))).toEqual([audioId, videoId]);
  });

  it("derives public formula/media text without treating these cards as empty", () => {
    const mathOnly = doc([{ type: "blockMath", attrs: { latex: "x=2" } }]);
    expect(extractRichDocumentText(mathOnly)).toBe("\\[x=2\\]");
    expect(isDocumentEmpty("", mathOnly)).toBe(false);
    const mixed = doc([
      { type: "paragraph", content: [
        { type: "text", text: "Solve " },
        { type: "inlineMath", attrs: { latex: "x^2=4" } },
      ] },
      { type: "audio", attrs: { ...media("audio", audioId).attrs, title: "secret answer" } },
      media("video", videoId),
    ]);
    expect(extractRichDocumentText(mixed)).toBe("Solve \\(x^2=4\\)\n[Audio]\n[Video]");
    expect(isDocumentEmpty("", mixed)).toBe(false);
    expect(isDocumentEmpty("", doc([{ type: "paragraph" }]))).toBe(true);
    expect(extractRichDocumentText(doc([{ type: "codeBlock", content: [
      { type: "text", text: "\\(literal code\\)" },
    ] }]))).toBe("\\(literal code\\)");
  });
});

const editors: Editor[] = [];
afterEach(() => {
  for (const editor of editors.splice(0)) editor.destroy();
});

function openEditor(document: RichDocument, assets: Asset[] = []): Editor {
  const editor = new Editor({
    element: globalThis.document.createElement("div"),
    extensions: createEditorExtensions(),
    content: hydrateDocument(canonicalizeDocument(document), assets),
  });
  editors.push(editor);
  return editor;
}

function editorAtoms(editor: Editor) {
  const atoms: unknown[] = [];
  editor.state.doc.descendants((node) => {
    if (["inlineMath", "blockMath", "audio", "video"].includes(node.type.name)) {
      atoms.push(node.toJSON());
    }
  });
  return atoms;
}

describe("math and media through the actual Tiptap editor", () => {
  it("retains all four atoms when editing unrelated text, saving and reopening", () => {
    const source = doc([
      { type: "paragraph", content: [
        { type: "text", text: "Solve " },
        { type: "inlineMath", attrs: { latex: "x^2=4" } },
      ] },
      { type: "blockMath", attrs: { latex: "\\frac{a}{b}" } },
      { type: "audio", attrs: { ...media("audio", audioId).attrs, title: "private-title" } },
      { type: "video", attrs: { ...media("video", videoId).attrs, width: 640, height: 360 } },
    ]);
    const assets = [asset(audioId, "audio/mpeg"), asset(videoId, "video/mp4")];
    const editor = openEditor(source, assets);
    const before = editorAtoms(editor);
    expect(before).toHaveLength(4);
    editor.view.dispatch(editor.state.tr.insertText("Please ", 1));
    expect(editorAtoms(editor)).toEqual(before);
    const adapter = createTiptapEditorAdapter(editor);
    const saved = adapter.document();
    expect(saved.attrs).toEqual({ schemaVersion: 3 });
    expect(collectAssetIds(saved)).toEqual([audioId, videoId]);
    expect(JSON.stringify(saved)).not.toContain("ostudy-asset.localhost");
    expect(adapter.plainText()).toContain("Please Solve \\(x^2=4\\)");
    expect(adapter.plainText()).toContain("\\[\\frac{a}{b}\\]");
    expect(adapter.plainText()).toContain("[Audio]");
    expect(adapter.plainText()).toContain("[Video]");
    expect(adapter.plainText()).not.toContain("private-title");
    const reopened = openEditor(saved, assets);
    expect(editorAtoms(reopened)).toEqual(before);
    expect(createTiptapEditorAdapter(reopened).document()).toEqual(saved);
  });

  it("preserves nested atoms and old schema-two documents", () => {
    const source = doc([
      { type: "blockquote", content: [{ type: "blockMath", attrs: { latex: "a+b" } }] },
      { type: "bulletList", content: [{ type: "listItem", content: [
        { type: "paragraph", content: [{ type: "inlineMath", attrs: { latex: "c" } }] },
        media("audio", audioId),
      ] }] },
      { type: "table", content: [{ type: "tableRow", content: [
        { type: "tableCell", content: [media("video", videoId)] },
      ] }] },
    ]);
    const editor = openEditor(source);
    expect(editorAtoms(editor)).toHaveLength(4);
    const saved = createTiptapEditorAdapter(editor).document();
    expect(editorAtoms(openEditor(saved))).toEqual(editorAtoms(editor));
    const legacy = openEditor(doc([{ type: "paragraph", content: [
      { type: "text", text: "Legacy text", marks: [{ type: "bold" }] },
    ] }], 2));
    expect(createTiptapEditorAdapter(legacy).document().attrs).toEqual({ schemaVersion: 2 });
    expect(legacy.getHTML()).toContain("<strong>Legacy text</strong>");
  });

  it("keeps formula source inert and media URLs and titles out of the DOM", () => {
    const hostile = '<img src="https://example.invalid/leak" onerror="alert(1)">';
    const editor = openEditor(doc([
      { type: "paragraph", content: [{ type: "inlineMath", attrs: { latex: hostile } }] },
      { type: "audio", attrs: { ...media("audio", audioId).attrs, title: "secret-answer" } },
      media("video", videoId),
    ]), [asset(audioId, "audio/mpeg"), asset(videoId, "video/mp4")]);
    const surface = editor.view.dom;
    expect(surface.querySelector("img, audio, video, source, iframe, script, a")).toBeNull();
    expect(surface.querySelector("[data-study-math]")?.textContent).toBe(`\\(${hostile}\\)`);
    expect(surface.textContent).toContain("playback unavailable");
    expect(editor.getHTML()).not.toContain("ostudy-asset.localhost");
    expect(editor.getHTML()).not.toContain("secret-answer");
    expect(editor.getHTML()).not.toContain("<img");
    expect(JSON.stringify(createTiptapEditorAdapter(editor).document())).toContain("secret-answer");
  });

  it("round-trips formula clipboard HTML without accepting arbitrary media tags", () => {
    const source = openEditor(doc([
      { type: "paragraph", content: [{ type: "inlineMath", attrs: { latex: "x<y & z" } }] },
      { type: "blockMath", attrs: { latex: "\\sqrt{x}" } },
    ]));
    const target = openEditor(doc([{ type: "paragraph" }], 2));
    target.commands.setContent(source.getHTML());
    expect(editorAtoms(target)).toEqual(editorAtoms(source));
    expect(createTiptapEditorAdapter(target).document().attrs).toEqual({ schemaVersion: 3 });
    target.commands.setContent('<audio src="https://example.invalid/leak"></audio>');
    expect(editorAtoms(target)).toHaveLength(0);
    expect(target.view.dom.querySelector("audio")).toBeNull();
  });

  it("does not attach forbidden marks to atoms when formatting across a formula", () => {
    const editor = openEditor(doc([{ type: "paragraph", content: [
      { type: "text", text: "Before " },
      { type: "inlineMath", attrs: { latex: "x" } },
      { type: "text", text: " after" },
    ] }]));
    const original = editorAtoms(editor);
    editor.commands.selectAll();
    editor.commands.toggleBold();
    expect(editorAtoms(editor)).toEqual(original);
    expect(editor.getHTML()).toContain("<strong>Before </strong>");
    expect(editor.getHTML()).toContain("<strong> after</strong>");
    expect(editor.commands.undo()).toBe(true);
    expect(editorAtoms(editor)).toEqual(original);
    expect(editor.getHTML()).not.toContain("<strong>");
    expect(editor.commands.redo()).toBe(true);
    expect(editorAtoms(editor)).toEqual(original);
  });
});

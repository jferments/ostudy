import { describe, expect, it, vi } from "vitest";

import type { Asset } from "../ipc/assets";
import type { RichDocument } from "../ipc/common";
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

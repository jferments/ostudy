import { convertFileSrc } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { makeAsset, paragraphDocument } from "../test/factories";
import {
  canonicalizeDocument,
  cloneDocument,
  collectAssetIds,
  hydrateDocument,
  isDocumentEmpty,
} from "./richText";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: vi.fn((path: string) => `asset-url:${path}`),
}));

describe("rich document helpers", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("canonicalizes managed images without mutating editor state", () => {
    const source = {
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            assetId: "a".repeat(32),
            src: "asset-url:/tmp/diagram.png",
            width: "640",
            height: 20_000,
          },
        },
      ],
    };

    const canonical = canonicalizeDocument(source);

    expect(canonical).not.toBe(source);
    expect(canonical).toEqual({
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            assetId: "a".repeat(32),
            src: `study-asset://${"a".repeat(32)}`,
            width: 640,
            height: null,
          },
        },
      ],
    });
    expect(source.content[0]?.attrs.src).toBe("asset-url:/tmp/diagram.png");
  });

  it("hydrates only known managed assets and supplies missing alt text", () => {
    const asset = makeAsset({
      id: "b".repeat(32),
      originalFilename: "heart.png",
      filePath: "/managed/heart.png",
    });
    const document = {
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            assetId: asset.id,
            src: `study-asset://${asset.id}`,
            alt: "",
          },
        },
        {
          type: "image",
          attrs: {
            assetId: "c".repeat(32),
            src: `study-asset://${"c".repeat(32)}`,
            alt: "Unresolved",
          },
        },
      ],
    };

    const hydrated = hydrateDocument(document, [asset]);

    expect(convertFileSrc).toHaveBeenCalledWith(asset.filePath);
    expect(hydrated).toEqual({
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            assetId: asset.id,
            src: `asset-url:${asset.filePath}`,
            alt: asset.originalFilename,
          },
        },
        {
          type: "image",
          attrs: {
            assetId: "c".repeat(32),
            src: `study-asset://${"c".repeat(32)}`,
            alt: "Unresolved",
          },
        },
      ],
    });
  });

  it("collects unique image identities from nested documents", () => {
    const first = "d".repeat(32);
    const second = "e".repeat(32);
    expect(
      collectAssetIds({
        type: "doc",
        content: [
          { type: "image", attrs: { assetId: first } },
          {
            type: "blockquote",
            content: [
              { type: "image", attrs: { assetId: second } },
              { type: "image", attrs: { assetId: first } },
            ],
          },
        ],
      }),
    ).toEqual([first, second]);
  });

  it("treats image-only documents as non-empty", () => {
    expect(isDocumentEmpty("", paragraphDocument(""))).toBe(true);
    expect(
      isDocumentEmpty("", {
        type: "doc",
        content: [{ type: "image", attrs: { assetId: "f".repeat(32) } }],
      }),
    ).toBe(false);
    expect(isDocumentEmpty("Visible text", paragraphDocument("Visible text"))).toBe(
      false,
    );
  });

  it("deep-clones a document before callers mutate it", () => {
    const original = paragraphDocument("Original");
    const clone = cloneDocument(original);
    const paragraph = (clone.content as Array<Record<string, unknown>>)[0];
    const text = (paragraph?.content as Array<Record<string, unknown>>)[0];
    if (text) text.text = "Changed";

    expect(original).toEqual(paragraphDocument("Original"));
    expect(clone).toEqual(paragraphDocument("Changed"));
  });
});

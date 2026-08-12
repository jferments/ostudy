#!/usr/bin/env node

import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

const root = fileURLToPath(new URL("..", import.meta.url));
await writeFile(
  join(root, "src/editor/richText.test.ts"),
  `import { beforeEach, describe, expect, it, vi } from "vitest";

import { managedAssetUrl } from "../platform/assets";
import { makeAsset, paragraphDocument } from "../test/factories";
import {
  canonicalizeDocument,
  cloneDocument,
  collectAssetIds,
  hydrateDocument,
  isDocumentEmpty,
} from "./richText";

vi.mock("../platform/assets", () => ({
  managedAssetUrl: vi.fn((assetId: string) => \`asset-url:\${assetId}\`),
}));

describe("rich document helpers", () => {
  beforeEach(() => vi.clearAllMocks());

  it("canonicalizes managed images without mutating editor state", () => {
    const source = {
      type: "doc",
      content: [{ type: "image", attrs: { assetId: "a".repeat(32), src: "temporary", width: "640", height: 20_000 } }],
    };
    const canonical = canonicalizeDocument(source);
    expect(canonical).not.toBe(source);
    expect(canonical.content?.[0]?.attrs).toMatchObject({
      assetId: "a".repeat(32),
      src: \`study-asset://\${"a".repeat(32)}\`,
      width: 640,
      height: null,
    });
    expect(source.content[0]?.attrs.src).toBe("temporary");
  });

  it("hydrates known asset IDs through the managed protocol", () => {
    const asset = makeAsset({ id: "b".repeat(32), originalFilename: "heart.png" });
    const hydrated = hydrateDocument(
      { type: "doc", content: [{ type: "image", attrs: { assetId: asset.id, src: \`study-asset://\${asset.id}\`, alt: "" } }] },
      [asset],
    );
    expect(managedAssetUrl).toHaveBeenCalledWith(asset.id);
    expect(hydrated.content?.[0]?.attrs).toMatchObject({
      src: \`asset-url:\${asset.id}\`,
      alt: "heart.png",
    });
  });

  it("collects unique asset IDs and treats image-only documents as non-empty", () => {
    const first = "d".repeat(32);
    const second = "e".repeat(32);
    const document = {
      type: "doc",
      content: [
        { type: "image", attrs: { assetId: first } },
        { type: "blockquote", content: [{ type: "image", attrs: { assetId: second } }, { type: "image", attrs: { assetId: first } }] },
      ],
    };
    expect(collectAssetIds(document)).toEqual([first, second]);
    expect(isDocumentEmpty("", document)).toBe(false);
    expect(isDocumentEmpty("", paragraphDocument(""))).toBe(true);
  });

  it("deep-clones documents", () => {
    const original = paragraphDocument("Original");
    const clone = cloneDocument(original);
    const paragraph = clone.content?.[0];
    const text = paragraph?.content?.[0];
    if (text) text.text = "Changed";
    expect(original).toEqual(paragraphDocument("Original"));
    expect(clone).toEqual(paragraphDocument("Changed"));
  });
});
`,
  "utf8",
);

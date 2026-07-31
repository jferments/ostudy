import { describe, expect, it } from "vitest";

import { canonicalizeDocument } from "./richText";

const ASSET_ID = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

describe("canonicalizeDocument image layout", () => {
  it("keeps only supported image size and alignment values", () => {
    const document = canonicalizeDocument({
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            assetId: ASSET_ID,
            src: "https://untrusted.example/image.png",
            size: "calc(100% + 1px)",
            alignment: "fixed",
            width: "320",
            height: null,
          },
        },
      ],
    });

    const image = (document.content as Array<Record<string, unknown>>)[0];
    const attrs = image.attrs as Record<string, unknown>;
    expect(attrs.src).toBe(`study-asset://${ASSET_ID}`);
    expect(attrs.size).toBe("medium");
    expect(attrs.alignment).toBe("center");
    expect(attrs.width).toBe(320);
    expect(attrs.height).toBeNull();
  });
});

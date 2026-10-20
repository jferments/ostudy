import { describe, expect, it, vi } from "vitest";

import type { DocumentPageText } from "../../../ipc/documentAnnotations";
import { characterAt, characterNear, createPageTextCache, textBetween, toPageText, type PageText } from "./textSelection";

/** Two lines of 6-point-wide characters, 10 points tall, with an implied line break between them. */
function twoLines(): PageText {
  const lines = ["heart rate", "output"];
  const characters: string[] = [];
  const boxes: number[] = [];
  lines.forEach((line, row) => {
    Array.from(line).forEach((character, column) => {
      characters.push(character);
      boxes.push(72 + column * 6, 100 + row * 14, 6, 10);
    });
    if (row === 0) {
      characters.push("\r", "\n");
      boxes.push(0, 0, 0, 0, 0, 0, 0, 0);
    }
  });
  return { characters, boxes: Float64Array.from(boxes) };
}

describe("page text", () => {
  it("scales the PDF's points to the annotator's page", () => {
    const text = toPageText(
      { width: 1224, height: 1584 },
      { pageNumber: 1, width: 612, height: 792, characters: "a𝑥", boxes: [10, 20, 5, 8, 30, 20, 6, 8] },
    );
    expect(text.characters).toEqual(["a", "𝑥"]);
    expect(Array.from(text.boxes)).toEqual([20, 40, 10, 16, 60, 40, 12, 16]);
  });

  it("finds the character under a point, and nothing off the text", () => {
    const text = twoLines();
    expect(characterAt(text, { x: 75, y: 105 }, 2)).toBe(0);
    expect(characterAt(text, { x: 87, y: 118 }, 2)).toBe(14);
    expect(characterAt(text, { x: 300, y: 300 }, 2)).toBeNull();
    // The space between words is drawn but empty: the nearest letter wins.
    expect(text.characters[characterAt(text, { x: 104, y: 105 }, 2) ?? -1]).toBe("t");
  });

  it("follows a drag past the end of a line on that line", () => {
    const text = twoLines();
    expect(characterNear(text, { x: 400, y: 105 })).toBe(9);
    expect(characterNear(text, { x: 10, y: 118 })).toBe(12);
  });

  it("covers each line with one box and joins lines with a space", () => {
    const text = twoLines();
    const forward = textBetween(text, 6, 14);
    expect(forward.text).toBe("rate out");
    expect(forward.rects).toEqual([108, 100, 24, 10, 72, 114, 18, 10]);
    expect(textBetween(text, 14, 6)).toEqual(forward);
  });
});

describe("page text cache", () => {
  const page = { pageNumber: 3, assetId: "a", width: 612, height: 792, imageAvailable: true };

  it("reads a page's text once, and remembers a page without text", async () => {
    const load = vi.fn((_documentId: string, pageNumber: number): Promise<DocumentPageText> => pageNumber === 3
      ? Promise.resolve({ pageNumber, width: 612, height: 792, characters: "a", boxes: [1, 2, 3, 4] })
      : Promise.reject(new Error("no text")));
    const cache = createPageTextCache("d".repeat(32), load);
    cache.request(page);
    cache.request(page);
    cache.request({ ...page, pageNumber: 4 });
    expect(cache.get(3)).toBeNull();
    await vi.waitFor(() => expect(cache.get(3)?.characters).toEqual(["a"]));
    expect(cache.get(4)).toBeNull();
    cache.request({ ...page, pageNumber: 4 });
    expect(load).toHaveBeenCalledTimes(2);
    cache.dispose();
  });
});

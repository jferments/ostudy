import { describe, expect, it } from "vitest";

import {
  distinctClozeGroups,
  markSelectionAsBlank,
  removeAllClozeMarkers,
  renderClozePreview,
} from "./cloze";

describe("fill-in-the-blank authoring", () => {
  it("marks selected phrases with stable increasing blank numbers", () => {
    const original =
      "ACE inhibitors block the enzyme that creates angiotensin II, lowering blood pressure.";
    const aceStart = original.indexOf("ACE inhibitors");
    const first = markSelectionAsBlank(
      original,
      aceStart,
      aceStart + "ACE inhibitors".length,
    );
    const angiotensinStart = first.source.indexOf("angiotensin II");
    const second = markSelectionAsBlank(
      first.source,
      angiotensinStart,
      angiotensinStart + "angiotensin II".length,
    );

    expect(second.source).toContain("{{c1::ACE inhibitors}}");
    expect(second.source).toContain("{{c2::angiotensin II}}");
    expect(distinctClozeGroups(second.source)).toEqual([1, 2]);
  });

  it("hides one blank at a time or all blanks together", () => {
    const source =
      "{{c1::ACE inhibitors}} block the enzyme that creates {{c2::angiotensin II}}.";

    const firstOnly = renderClozePreview(source, 1);
    expect(firstOnly.question).toBe("[…] block the enzyme that creates angiotensin II.");
    expect(firstOnly.answer).toContain("ACE inhibitors");
    expect(firstOnly.answer).not.toMatch(/^angiotensin II/u);

    const allTogether = renderClozePreview(source, null);
    expect(allTogether.question).toBe("[…] block the enzyme that creates […].");
    expect(allTogether.answer).toContain("ACE inhibitors · angiotensin II");
    expect(allTogether.answer).toContain(
      "Complete statement: ACE inhibitors block the enzyme that creates angiotensin II.",
    );
  });

  it("removes marker syntax without removing the selected text", () => {
    expect(removeAllClozeMarkers("A {{c1::beta blocker}} lowers heart rate.")).toBe(
      "A beta blocker lowers heart rate.",
    );
  });

  it("rejects empty selections and nested markers", () => {
    expect(() => markSelectionAsBlank("No selection", 2, 2)).toThrow(
      "Select one or more words",
    );
    expect(() => markSelectionAsBlank("{{c1::marked}}", 0, 14)).toThrow(
      "already contains a blank marker",
    );
  });
});

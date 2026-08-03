import { describe, expect, it } from "vitest";

import {
  distinctClozeGroups,
  findClozeBlanks,
  markSelectionAsBlank,
  nextClozeGroup,
  renderClozePreview,
} from "./cloze";

describe("fill-in-the-blank edge cases", () => {
  it("preserves whitespace outside the selected phrase", () => {
    const source = "Drug class:   ACE inhibitor   lowers blood pressure.";
    const start = source.indexOf("   ACE inhibitor   ");
    const end = start + "   ACE inhibitor   ".length;

    const result = markSelectionAsBlank(source, start, end);

    expect(result.source).toBe(
      "Drug class:   {{c1::ACE inhibitor}}   lowers blood pressure.",
    );
    expect(result.source.slice(result.selectionStart, result.selectionEnd)).toBe(
      "{{c1::ACE inhibitor}}",
    );
  });

  it("uses the highest existing group when numbering the next blank", () => {
    const source = "{{c2::Second}} then {{c7::Seventh}}";
    expect(nextClozeGroup(source)).toBe(8);
  });

  it("deduplicates repeated groups while retaining every answer", () => {
    const source =
      "The {{c2::right atrium}} and {{c2::left atrium}} are atria; {{c5::ventricles}} pump blood.";

    expect(distinctClozeGroups(source)).toEqual([2, 5]);
    expect(findClozeBlanks(source).map((blank) => blank.text)).toEqual([
      "right atrium",
      "left atrium",
      "ventricles",
    ]);
    expect(renderClozePreview(source, 2).answer).toContain(
      "right atrium · left atrium",
    );
  });

  it("leaves non-target groups visible in one-at-a-time preview", () => {
    const preview = renderClozePreview(
      "{{c1::Alpha}} plus {{c2::Beta}} plus {{c3::Gamma}}",
      2,
    );

    expect(preview.question).toBe("Alpha plus […] plus Gamma");
    expect(preview.answer).toContain("Beta");
    expect(preview.answer).toContain("Complete statement: Alpha plus Beta plus Gamma");
  });

  it("clamps reversed or out-of-range selections before marking", () => {
    const result = markSelectionAsBlank("Alpha Beta", 6, 99);
    expect(result.source).toBe("Alpha {{c1::Beta}}");
    expect(() => markSelectionAsBlank("Alpha", 99, 2)).toThrow(
      "Select one or more words",
    );
  });
});

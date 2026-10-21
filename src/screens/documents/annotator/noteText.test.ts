import { describe, expect, it } from "vitest";

import { continueList, isFormattedNote, noteLines, noteSpans, setListStyle, toggleCheck } from "./noteText";

describe("note text", () => {
  it("reads bullets, numbers and checkboxes, indented two spaces a level", () => {
    expect(noteLines("Heart\n- atria\n  * ventricles\n2) valves\n[ ] mitral\n- [x] aortic")).toEqual([
      { kind: "plain", indent: 0, body: "Heart" },
      { kind: "bullet", indent: 0, body: "atria" },
      { kind: "bullet", indent: 1, body: "ventricles" },
      { kind: "number", indent: 0, number: "2)", body: "valves" },
      { kind: "check", indent: 0, checked: false, body: "mitral" },
      { kind: "check", indent: 0, checked: true, body: "aortic" },
    ]);
    // A dash without a space is a word.
    expect(noteLines("-5 degrees")[0]).toMatchObject({ kind: "plain" });
  });

  it("finds formulas but leaves prices alone", () => {
    expect(noteSpans("Area $\\pi r^2$ costs $10")).toEqual([
      { text: "Area " }, { math: "\\pi r^2", block: false }, { text: " costs $10" },
    ]);
    expect(noteSpans("$$E = mc^2$$")).toEqual([{ math: "E = mc^2", block: true }]);
    expect(isFormattedNote("plain words, $5")).toBe(false);
    expect(isFormattedNote("- one")).toBe(true);
  });

  it("ticks and clears one checkbox", () => {
    expect(toggleCheck("[ ] a\n- [x] b", 0)).toBe("[x] a\n- [x] b");
    expect(toggleCheck("[ ] a\n- [x] b", 1)).toBe("[ ] a\n- [ ] b");
    expect(toggleCheck("plain", 0)).toBe("plain");
  });

  it("makes the chosen lines a list, and plain again", () => {
    const text = "Title\nfirst\nsecond";
    const listed = setListStyle(text, 7, 15, "number");
    expect(listed.text).toBe("Title\n1. first\n2. second");
    expect(setListStyle(listed.text, 7, listed.end, "number").text).toBe(text);
    expect(setListStyle("- a", 0, 0, "check").text).toBe("[ ] a");
  });

  it("continues a list on Enter and ends it on an empty item", () => {
    expect(continueList("1. one", 6)).toEqual({ text: "1. one\n2. ", caret: 10 });
    expect(continueList("  - one", 7)).toEqual({ text: "  - one\n  - ", caret: 12 });
    expect(continueList("- [x] done", 10)).toEqual({ text: "- [x] done\n- [ ] ", caret: 17 });
    expect(continueList("- one\n- ", 8)).toEqual({ text: "- one\n", caret: 6 });
    expect(continueList("plain", 5)).toBeNull();
  });
});

import { describe, expect, it } from "vitest";

import { detectQuizletSeparators, parseQuizletText } from "./quizletImport";

describe("Quizlet Export text", () => {
  it("reads Quizlet's default tab and new-line export", () => {
    const text = "hablar\tto speak\r\ncomer\tto eat, to dine\n\nvivir\t\n";
    expect(detectQuizletSeparators(text)).toEqual({ term: "tab", row: "newline" });
    expect(parseQuizletText(text, { term: "tab", row: "newline" })).toEqual({
      cards: [
        { question: "hablar", answer: "to speak" },
        { question: "comer", answer: "to eat, to dine" },
      ],
      skipped: 1,
    });
  });

  it("reads comma and semicolon exports, splitting each card at its first comma", () => {
    const text = "mitochondria,powerhouse of the cell;ATP,energy, mostly;";
    expect(detectQuizletSeparators(text)).toEqual({ term: "comma", row: "semicolon" });
    expect(parseQuizletText(text, { term: "comma", row: "semicolon" }).cards).toEqual([
      { question: "mitochondria", answer: "powerhouse of the cell" },
      { question: "ATP", answer: "energy, mostly" },
    ]);
  });

  it("keeps multi-line definitions when cards are separated by semicolons", () => {
    expect(parseQuizletText("Krebs cycle\tstep one\nstep two;", { term: "tab", row: "semicolon" }).cards)
      .toEqual([{ question: "Krebs cycle", answer: "step one\nstep two" }]);
  });
});

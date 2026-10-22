import { describe, expect, it } from "vitest";

import type { LessonBlock } from "../../ipc/lessons";
import { MAX_SLIDE_CARDS, slideCards } from "./slideCards";

describe("slideCards", () => {
  it("turns terms, misconceptions, comparisons and worked examples into cards", () => {
    const blocks: LessonBlock[] = [
      { kind: "explanation", heading: null, body: "ACE turns angiotensin I into angiotensin II." },
      { kind: "term", term: "Renin", explanation: "An enzyme the kidney releases when pressure falls." },
      { kind: "misconception", misconception: "ACE inhibitors raise renin by accident.", correction: "Less angiotensin II takes the brake off renin." },
      { kind: "comparison", subjectA: "ACE inhibitors", subjectB: "ARBs", relationship: "Both lower angiotensin II's effect; only ACE inhibitors raise bradykinin." },
      { kind: "workedExample", title: "Cough", prompt: "Why does lisinopril cause a dry cough?", steps: [], answer: "Bradykinin builds up.", takeaway: null },
      { kind: "keyPoints", items: ["Renin rises"] },
    ];
    expect(slideCards({ blocks })).toEqual([
      { question: "Renin", answer: "An enzyme the kidney releases when pressure falls.", tags: [] },
      { question: "True or false: ACE inhibitors raise renin by accident.", answer: "False. Less angiotensin II takes the brake off renin.", tags: [] },
      { question: "How does ACE inhibitors relate to ARBs?", answer: "Both lower angiotensin II's effect; only ACE inhibitors raise bradykinin.", tags: [] },
      { question: "Why does lisinopril cause a dry cough?", answer: "Bradykinin builds up.", tags: [] },
    ]);
  });

  it("offers a question once, skips empty ones and stops at the limit", () => {
    const blocks: LessonBlock[] = [
      { kind: "term", term: "Renin", explanation: "An enzyme." },
      { kind: "term", term: "renin ", explanation: "Again." },
      { kind: "term", term: " ", explanation: "No term." },
      ...Array.from({ length: 12 }, (_, index): LessonBlock => ({ kind: "term", term: `Term ${index}`, explanation: "Meaning." })),
    ];
    const cards = slideCards({ blocks });
    expect(cards).toHaveLength(MAX_SLIDE_CARDS);
    expect(cards.filter((card) => card.question.toLowerCase() === "renin")).toHaveLength(1);
  });
});

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { TutorQuestion, TutorTurn } from "../../ipc/tutor";
import { earlierOutcomes, TutorQuizCard, type TutorQuizCardProps } from "./TutorQuizCard";

function turn(id: string, fields: Partial<TutorTurn>): TutorTurn {
  return {
    id: id.repeat(32), operationId: id.repeat(32), sequence: 1, mode: "quiz", userText: "",
    assistantText: "", state: "complete", citations: [], evidenceCurrent: true, pendingQuestion: false,
    feedback: null, question: null, questionId: null, provider: "fixture", model: "fixture", errorCode: "",
    errorMessage: "", totalTokens: null, createdAt: 1, replacedByTurnId: null, attachments: [], ...fields,
  };
}

const choose: TutorQuestion = { number: 2, total: 4, choices: ["Gastrin", "Secretin", "Motilin", "GIP"], answered: false };
const asked = turn("a", { assistantText: "Which hormone is most likely deficient?", pendingQuestion: true, question: choose });

function renderCard(props: Partial<TutorQuizCardProps> = {}) {
  const handlers = { onChoose: vi.fn(), onNext: vi.fn(), onExplain: vi.fn(), onSave: vi.fn() };
  render(
    <TutorQuizCard turn={asked} answer={null} earlier={["correct"]} latest disabled={false} citations={null} {...handlers} {...props} />,
  );
  return handlers;
}

describe("Tutor quiz cards", () => {
  it("asks a choose-one question with its place in the run", () => {
    const { onChoose } = renderCard();
    expect(screen.getByRole("article", { name: "Question 2" })).toBeTruthy();
    expect(screen.getByText("Choose one")).toBeTruthy();
    expect(screen.getByRole("img", { name: "Question 2 of 4" })).toBeTruthy();
    const check = screen.getByRole("button", { name: "Check answer" }) as HTMLButtonElement;
    expect(check.disabled).toBe(true);
    fireEvent.click(screen.getByRole("radio", { name: /Secretin/u }));
    fireEvent.click(check);
    expect(onChoose).toHaveBeenCalledWith(1);
    expect(screen.queryByRole("button", { name: "Next question" })).toBeNull();
  });

  it("marks the picked and the right option once answered, and offers what comes next", () => {
    const answer = turn("b", {
      mode: "answer", userText: "Gastrin", questionId: asked.id,
      feedback: { outcome: "incorrect", explanation: "", criteriaFeedback: [], criteriaMet: [], chosen: 0 },
    });
    const { onNext, onExplain, onSave } = renderCard({
      turn: { ...asked, pendingQuestion: false, question: { ...choose, answered: true, correctChoice: 1, expectedAnswer: "Secretin", explanation: "Secretin brings bicarbonate." } },
      answer,
    });
    expect(screen.getByRole("article", { name: "Question 2, answered" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: /Secretin, the right answer/u })).toBeTruthy();
    expect(screen.getByRole("radio", { name: /Gastrin, your answer/u })).toBeTruthy();
    expect(screen.getByText("Not yet correct")).toBeTruthy();
    expect(screen.getByText("Secretin brings bicarbonate.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Check answer" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Next question" }));
    fireEvent.click(screen.getByRole("button", { name: "Explain what I missed" }));
    fireEvent.click(screen.getByRole("button", { name: "Save as flashcard" }));
    expect(onNext).toHaveBeenCalled();
    expect(onExplain).toHaveBeenCalled();
    expect(onSave).toHaveBeenCalled();
  });

  it("lists what a short answer met and missed, and ends with the last question", () => {
    const question: TutorQuestion = {
      number: 3, total: 3, choices: [], answered: true, expectedAnswer: "Ductal cells; bicarbonate", explanation: "Secretin acts on the ducts.",
    };
    const answer = turn("c", {
      mode: "answer", userText: "Ductal cells and enzymes", questionId: asked.id,
      feedback: {
        outcome: "partial", explanation: "Half there.", criteriaFeedback: ["Names the ductal cells", "Enzymes come from acinar cells"],
        criteriaMet: [true, false], chosen: null,
      },
    });
    renderCard({ turn: { ...asked, question }, answer });
    expect(screen.getByText("Short answer")).toBeTruthy();
    expect(screen.getByText("Ductal cells and enzymes")).toBeTruthy();
    expect(screen.getByText("Partly correct")).toBeTruthy();
    const needed = screen.getByRole("list", { name: "What the answer needed" });
    expect(needed.querySelectorAll('[aria-label="Met"]').length).toBe(1);
    expect(needed.querySelectorAll('[aria-label="Missed"]').length).toBe(1);
    expect(screen.getByText("Ductal cells; bicarbonate")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Next question" })).toBeNull();
    expect(screen.getByRole("button", { name: "Explain what I missed" })).toBeTruthy();
  });

  it("follows the run back through its earlier questions", () => {
    const first = turn("1", { question: { number: 1, total: 3, choices: [], answered: true } });
    const graded = turn("2", { mode: "answer", questionId: first.id, feedback: { outcome: "correct", explanation: "", criteriaFeedback: [], criteriaMet: [], chosen: null } });
    const talk = turn("3", { mode: "explain" });
    const second = turn("4", { question: { number: 2, total: 3, choices: [], answered: false } });
    const third = turn("5", { question: { number: 3, total: 3, choices: [], answered: false } });
    expect(earlierOutcomes([first, graded, talk, second, third], 4)).toEqual(["correct", null]);
    expect(earlierOutcomes([first], 0)).toEqual([]);
  });
});

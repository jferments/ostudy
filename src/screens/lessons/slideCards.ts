import type { BulkCardRowInput } from "../../ipc/cards";
import type { LessonStep } from "../../ipc/lessons";

/** At most this many cards are offered for one slide. */
export const MAX_SLIDE_CARDS = 8;

/**
 * The cards a step's own blocks make, in block order: a term and what it
 * means, a misconception set right, how two things relate, and a worked
 * example's answer. Other blocks teach rather than test, so they make none.
 * Repeated questions are offered once.
 */
export function slideCards(step: Pick<LessonStep, "blocks">): BulkCardRowInput[] {
  const cards: BulkCardRowInput[] = [];
  const seen = new Set<string>();
  const add = (question: string, answer: string) => {
    const front = question.trim();
    const back = answer.trim();
    const key = front.toLocaleLowerCase();
    if (front === "" || back === "" || seen.has(key) || cards.length >= MAX_SLIDE_CARDS) return;
    seen.add(key);
    cards.push({ question: front, answer: back, tags: [] });
  };
  for (const block of step.blocks) {
    switch (block.kind) {
      case "term":
        add(block.term, block.explanation);
        break;
      case "misconception":
        add(`True or false: ${block.misconception.trim()}`, `False. ${block.correction.trim()}`);
        break;
      case "comparison":
        add(`How does ${block.subjectA.trim()} relate to ${block.subjectB.trim()}?`, block.relationship);
        break;
      case "workedExample":
        add(block.prompt, block.answer);
        break;
      default:
        break;
    }
  }
  return cards;
}

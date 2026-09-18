import { isInteractivePracticePresentation, type PracticeCard } from "../../ipc/practice";
import type { PracticeAttemptView, PracticeQuestion } from "../../ipc/questionAttempts";

/** A late acknowledgement belongs to its captured identity, never whichever
 * array position happens to be active by the time the native call returns.
 */
export function mergeQuestionAttempt(cards: PracticeCard[], attempt: PracticeAttemptView): PracticeCard[] {
  let changed = false;
  const next = cards.map((card) => {
    if (!isInteractivePracticePresentation(card.presentation)) return card;
    const previous = card.presentation.question.attempt;
    if (previous.instanceId !== attempt.instanceId || previous.question.kind !== attempt.question.kind
      || previous.revision >= attempt.revision) return card;
    changed = true;
    return { ...card, presentation: { ...card.presentation, question: { ...card.presentation.question, attempt } } };
  });
  return changed ? next : cards;
}

/** Same-revision reconciliation may add visible media, but must not roll an
 * exposed answer back to an earlier phase or substitute a different question.
 */
export function mergePracticeQuestion(cards: PracticeCard[], question: PracticeQuestion): PracticeCard[] {
  let changed = false;
  const next = cards.map((card) => {
    if (!isInteractivePracticePresentation(card.presentation)) return card;
    const previous = card.presentation.question.attempt;
    const incoming = question.attempt;
    if (previous.instanceId !== incoming.instanceId || previous.question.kind !== incoming.question.kind
      || previous.revision > incoming.revision
      || (previous.revision === incoming.revision && previous.progress.phase !== incoming.progress.phase)) return card;
    changed = true;
    return { ...card, presentation: { ...card.presentation, question } };
  });
  return changed ? next : cards;
}

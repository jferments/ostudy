import type { ComponentFeedback, PublicQuestion, QuestionResponse } from "../../ipc/questionAttempts";

export type Placement = { itemId: string; targetId: string };
export type PlacementFeedback = Extract<ComponentFeedback, { kind: "placement" }>;
export const answerTile = "tw:min-w-0 tw:rounded-xl tw:border tw:border-solid tw:p-3 tw:text-foreground";
export const neutralTile = "tw:border-border tw:bg-background";
export const correctTile = "tw:border-success/60 tw:bg-success-soft";
export const incorrectTile = "tw:border-destructive/60 tw:bg-destructive-soft";

export function tileResult(correct: boolean | undefined): string {
  return correct === undefined ? neutralTile : correct ? correctTile : incorrectTile;
}

export function emptyQuestionResponse(question: PublicQuestion): QuestionResponse {
  switch (question.kind) {
    case "single_choice": return { kind: "single_choice", selectedOptionId: null };
    case "select_all": return { kind: "select_all", selectedOptionIds: [] };
    case "true_false": return { kind: "true_false", selected: null };
    case "matching": return { kind: "matching", assignments: [] };
    case "grouping": return { kind: "grouping", assignments: [] };
    case "ordering": return { kind: "ordering", orderedItemIds: question.items.map((item) => item.id) };
  }
}

/** An explicit connection owns its row. Loose tiles fill only unclaimed rows. */
export function matchingRows(itemIds: readonly string[], targetIds: readonly string[], assignments: readonly Placement[]): string[] {
  const connected = new Map(assignments.map((pair) => [pair.targetId, pair.itemId]));
  const used = new Set(assignments.map((pair) => pair.itemId));
  const loose = itemIds.filter((id) => !used.has(id));
  let cursor = 0;
  return targetIds.map((id) => connected.get(id) ?? loose[cursor++]);
}

/** Sorting onto a row and dropping onto its opposite tile have identical meaning.
 * A connection cannot silently displace another connection: unlink it first.
 */
export function connectMatching(assignments: readonly Placement[], itemId: string, targetId: string): Placement[] | null {
  if (assignments.some((pair) => pair.itemId === itemId || pair.targetId === targetId)) return null;
  return [...assignments, { itemId, targetId }];
}

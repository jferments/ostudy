import type { ComponentFeedback, PublicQuestion, QuestionResponse } from "../../ipc/questionAttempts";

export type Placement = { itemId: string; targetId: string };
export type PlacementFeedback = Extract<ComponentFeedback, { kind: "placement" }>;
export const answerTile = "tw:min-w-0 tw:rounded-md tw:border tw:border-solid tw:p-3 tw:text-foreground";
export const neutralTile = "tw:border-border tw:bg-background";
export const connectedTile = "tw:border-primary/60 tw:bg-accent";
export const correctTile = "tw:border-success/60 tw:bg-success-soft";
export const incorrectTile = "tw:border-destructive/60 tw:bg-destructive-soft";
/** Solid semantic colors keep both the moving tile and its destination readable.
 * Rich descendants inherit the active foreground instead of retaining an accent
 * color that disappears against the active background.
 */
export const activePointerTile = "tw:border-primary tw:bg-primary tw:text-primary-foreground tw:[&_*]:text-inherit";
/** The real button that covers a tile. It sits under the tile's content, which lets pointers
 * through except on the content's own links and players. */
export const tileHandle = "tw:absolute tw:inset-0 tw:h-auto tw:w-auto tw:rounded-[inherit] tw:border-0 tw:bg-transparent tw:p-0 tw:enabled:hover:bg-foreground/5";
export const tileContent = "tw:relative tw:min-w-0 tw:[&_:is(a,button,audio,video,input,select,textarea,[contenteditable=true])]:pointer-events-auto";

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
  return targetIds.map((id) => {
    const connectedItem = connected.get(id);
    if (connectedItem) return connectedItem;
    const looseItem = loose[cursor];
    cursor += 1;
    if (!looseItem) throw new Error("Matching response does not contain enough unassigned items.");
    return looseItem;
  });
}

/** New connections never displace another pair implicitly. */
export function connectMatching(assignments: readonly Placement[], itemId: string, targetId: string): Placement[] | null {
  if (assignments.some((pair) => pair.itemId === itemId || pair.targetId === targetId)) return null;
  return [...assignments, { itemId, targetId }];
}

/** An explicit rematch changes only the two involved rows. When both are paired,
 * swap their targets; when the dragged tile was loose, release the displaced tile.
 * All unrelated connections keep their identities and positions.
 */
export function rematchMatching(assignments: readonly Placement[], itemId: string, targetId: string): Placement[] | null {
  const previous = assignments.find((pair) => pair.itemId === itemId);
  if (previous?.targetId === targetId) return null;
  const displaced = assignments.find((pair) => pair.targetId === targetId);
  const next = assignments.filter((pair) => pair.itemId !== itemId && pair.targetId !== targetId);
  if (previous && displaced) next.push({ itemId: displaced.itemId, targetId: previous.targetId });
  next.push({ itemId, targetId });
  return next;
}

export function responseReady(response: QuestionResponse): boolean {
  if (response.kind === "single_choice") return response.selectedOptionId !== null;
  if (response.kind === "true_false") return response.selected !== null;
  if (response.kind === "matching" || response.kind === "grouping") return response.assignments.length > 0;
  if (response.kind === "ordering") return response.orderedItemIds.length > 0;
  return true;
}

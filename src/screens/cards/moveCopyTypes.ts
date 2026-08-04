import type { SelectionCard } from "../../ipc";

/**
 * Move/copy acts on source notes, not independently scheduled presentations.
 * Search results can provide the generated-card ID, while the individual card
 * editor only needs the durable note and current deck identities.
 */
export type SourceNoteSelection = Pick<SelectionCard, "noteId" | "deckId"> &
  Partial<Pick<SelectionCard, "cardId">>;

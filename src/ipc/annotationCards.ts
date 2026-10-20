import { invoke } from "./transport";

/** The mark a card was made from: its page and a copy of the marked words. */
export interface CardMarkSource {
  noteId: string;
  /** Empty once the document is deleted; the card and its words stay. */
  documentId: string | null;
  /** Empty once the mark is erased. */
  annotationId: string | null;
  pageNumber: number;
  /** A text highlight's words or a note's text; empty for strokes and shapes. */
  quote: string;
}

export interface DocumentMarkedCards {
  documentId: string;
  /** Cards made from this document's marks, oldest first. */
  cardIds: string[];
  /** Where a new card from this document goes unless the learner picks another deck. */
  suggestedDeckId: string | null;
}

/** Link a saved card note to the mark it was made from; repeating it is harmless. */
export async function linkCardNoteToAnnotation(
  noteId: string,
  annotationId: string,
): Promise<CardMarkSource> {
  return invoke<CardMarkSource>("link_card_note_to_annotation", {
    input: { noteId, annotationId },
  });
}

export async function getDocumentMarkedCards(documentId: string): Promise<DocumentMarkedCards> {
  return invoke<DocumentMarkedCards>("get_document_marked_cards", { input: { documentId } });
}

/** The mark a card was made from, or null for a card not made from one. */
export async function getCardMarkSource(noteId: string): Promise<CardMarkSource | null> {
  return invoke<CardMarkSource | null>("get_card_mark_source", { input: { noteId } });
}

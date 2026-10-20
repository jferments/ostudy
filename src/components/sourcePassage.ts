import type { CardMarkSource } from "../ipc/annotationCards";
import type { SourceCitation } from "../ipc/documents";

/** Where a card's source opens: its document and the first cited page. */
export interface SourcePassage {
  documentId: string;
  title: string;
  pageNumber: number;
  /** The mark the card was made from, when it is still on that page. */
  annotationId?: string;
}

/**
 * The passage a citation can open, or null when it can only be read as text:
 * the document was removed (or never came with a shared deck), it is not a
 * PDF with pages to show, or the citation names no page.
 */
export function sourcePassageFor(citation: SourceCitation | null | undefined): SourcePassage | null {
  const locator = citation?.sourceLocator;
  if (!citation || citation.sourceRemoved || citation.sourceFormat !== "pdf" || !locator) return null;
  if (locator.unitKind !== "page" || locator.unitNumbers.length === 0) return null;
  return {
    documentId: citation.sourceDocumentId,
    title: citation.sourceTitle,
    pageNumber: Math.min(...locator.unitNumbers),
  };
}

/**
 * A card made from a mark opens at that mark, as long as the mark is still on
 * the cited document; otherwise it opens at the cited page.
 */
export function atMark(passage: SourcePassage | null, mark: CardMarkSource | null | undefined): SourcePassage | null {
  if (!passage || !mark?.annotationId || mark.documentId !== passage.documentId) return passage;
  return { ...passage, pageNumber: mark.pageNumber, annotationId: mark.annotationId };
}

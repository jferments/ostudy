import { invoke } from "./transport";

/** A box in PDF points from the top-left corner of the page as displayed. */
export interface FigureBox {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** A page whose text explains a figure. */
export interface FigurePassage {
  pageNumber: number;
  /** `mention` names the figure's label; `neighbour` is a page beside a figure that fills its own. */
  kind: "mention" | "neighbour";
  excerpt: string | null;
}

export interface DocumentFigure {
  figureId: string;
  pageNumber: number;
  pageWidth: number;
  pageHeight: number;
  kind: "image" | "drawing";
  bounds: FigureBox;
  captionBounds: FigureBox | null;
  /** Such as "Figure 3.2". */
  label: string | null;
  caption: string | null;
  hidden: boolean;
  passages: FigurePassage[];
}

export type DocumentFiguresState = "none" | "pending" | "found" | "unsupported" | "failed";

export interface DocumentFigures {
  documentId: string;
  /** The learner's choice at import; documents imported before it use their images. */
  useImages: boolean;
  state: DocumentFiguresState;
  scannedAt: number | null;
  figures: DocumentFigure[];
}

export async function getDocumentFigures(documentId: string): Promise<DocumentFigures> {
  return invoke<DocumentFigures>("get_document_figures", { input: { documentId } });
}

/** Searches the PDF for figures now, turning its images on. */
export async function findDocumentFigures(documentId: string): Promise<DocumentFigures> {
  return invoke<DocumentFigures>("find_document_figures", { input: { documentId } });
}

/** Turning images off forgets the figures found, hidden ones included. */
export async function setDocumentFiguresUseImages(
  documentId: string,
  useImages: boolean,
): Promise<DocumentFigures> {
  return invoke<DocumentFigures>("set_document_figures_use_images", { input: { documentId, useImages } });
}

export async function setDocumentFigureHidden(
  documentId: string,
  figureId: string,
  hidden: boolean,
): Promise<DocumentFigures> {
  return invoke<DocumentFigures>("set_document_figure_hidden", { input: { documentId, figureId, hidden } });
}

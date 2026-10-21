import { createOperationId } from "./common";
import type { PreparedExport } from "./externalFiles";
import { invoke } from "./transport";

/** Colors are `#rrggbb`; every length is in PDF points from the page's top-left corner. */
export interface InkShape {
  kind: "ink";
  /** `x, y, pressure` triples; pressure runs from 0 to 1. */
  points: number[];
  color: string;
  strokeWidth: number;
  /** Set when the input reported no pressure: the stroke is drawn from its speed instead. */
  simulatePressure: boolean;
  /** Marks of one page that share a group are selected and moved together. */
  group?: string;
}

export interface HighlightShape {
  kind: "highlight";
  /** `x, y` pairs of a free stroke; empty for a highlight of the page's text. */
  points: number[];
  color: string;
  strokeWidth: number;
  /** A text highlight's `x, y, width, height` boxes, one per line it covers. */
  rects?: number[];
  /** The words a text highlight covers. */
  text?: string;
  /** Marks of one page that share a group are selected and moved together. */
  group?: string;
}

/** How a line or outline is broken up; a solid line leaves `dash` out. */
export type LineDash = "dashed" | "dotted";

/** A line, or an arrow whose head is at the second point (and the first too with `bothEnds`). */
export interface SegmentShape {
  kind: "line" | "arrow";
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color: string;
  strokeWidth: number;
  dash?: LineDash;
  /** Only on an arrow. */
  bothEnds?: true;
  /** A curve: the control points `[x1, y1, x2, y2]` of a cubic Bézier from the first end to the second. */
  curve?: [number, number, number, number];
  /** Marks of one page that share a group are selected and moved together. */
  group?: string;
}

/** The outline of a rectangle, or of the ellipse inside it, filled with `fill` if it has one. */
export interface FrameShape {
  kind: "rectangle" | "ellipse";
  x: number;
  y: number;
  width: number;
  height: number;
  color: string;
  strokeWidth: number;
  dash?: LineDash;
  fill?: string;
  /** `false` drops the outline of a filled shape. */
  border?: false;
  /** Marks of one page that share a group are selected and moved together. */
  group?: string;
}

/** Typed text that wraps at `width`; its height follows the text. */
export interface TextShape {
  kind: "text";
  x: number;
  y: number;
  width: number;
  text: string;
  color: string;
  fontSize: number;
  /** A sticky note: a small note of `width` showing the start of its text; `color` is its paper. */
  sticky?: true;
  /** A sticky note folded down to a small icon; it still holds all of its text. */
  collapsed?: true;
  /** Only on a notebook page: the note is a link, and `text` is its label. */
  link?: NoteLink;
  /** Marks of one page that share a group are selected and moved together. */
  group?: string;
}

/** What a link on a notebook page opens. Targets are looked up when the notebook opens. */
export type LinkTarget =
  | { kind: "documentPage"; documentId: string; pageNumber: number }
  | { kind: "studyGuide"; documentId: string }
  | { kind: "deck"; deckId: string }
  | { kind: "notebookPage"; notebookId: string; pageId: string };

/** A link shown as a small link, or with `button` as a button that does the thing (a deck's starts Practice). */
export interface NoteLink {
  target: LinkTarget;
  button?: true;
}

/** A picture on a notebook page: a managed image drawn into its box. Documents hold none. */
export interface ImageShape {
  kind: "image";
  x: number;
  y: number;
  width: number;
  height: number;
  assetId: string;
  /** Marks of one page that share a group are selected and moved together. */
  group?: string;
}

export type AnnotationShape = InkShape | HighlightShape | SegmentShape | FrameShape | TextShape | ImageShape;
export type AnnotationKind = AnnotationShape["kind"];

export interface DocumentAnnotation {
  id: string;
  pageNumber: number;
  shape: AnnotationShape;
  revision: number;
  createdAt: number;
  updatedAt: number;
}

export interface AnnotatedPage {
  pageNumber: number;
  /** The page image; read it with `readDocumentSourceAsset`. */
  assetId: string;
  /** The page size in PDF points. */
  width: number;
  height: number;
  /** False when the page image could not be measured; the page then shows blank. */
  imageAvailable: boolean;
}

export interface DocumentAnnotations {
  documentId: string;
  pages: AnnotatedPage[];
  /** Earlier annotations first on each page. */
  annotations: DocumentAnnotation[];
}

/**
 * The new state of one annotation. A put creates it or replaces all of it, and
 * deleting one that is already gone succeeds, so a request can be repeated.
 */
export type DocumentAnnotationChange =
  | { op: "put"; id: string; pageNumber: number; shape: AnnotationShape }
  | { op: "delete"; id: string };

export interface AppliedDocumentAnnotationChanges {
  annotations: DocumentAnnotation[];
  deletedIds: string[];
}

/** The most changes one request may carry. */
export const MAX_ANNOTATION_CHANGES_PER_REQUEST = 500;

export async function getDocumentAnnotations(documentId: string): Promise<DocumentAnnotations> {
  return invoke<DocumentAnnotations>("get_document_annotations", { input: { documentId } });
}

export async function applyDocumentAnnotationChanges(
  documentId: string,
  changes: DocumentAnnotationChange[],
): Promise<AppliedDocumentAnnotationChanges> {
  return invoke<AppliedDocumentAnnotationChanges>("apply_document_annotation_changes", {
    input: { documentId, changes },
  });
}

/** One rectangle of a page drawn at `pageWidth` by `pageHeight` pixels; every number is a whole pixel. */
export interface DocumentPageTileRequest {
  pageNumber: number;
  pageWidth: number;
  pageHeight: number;
  left: number;
  top: number;
  width: number;
  height: number;
}

/** The longest side of a whole page drawn for tiles; one tile is at most 1,024 pixels a side. */
export const MAX_TILED_PAGE_SIDE = 16_384;

/** A PNG of part of a PDF page, drawn again from the PDF for a zoomed-in view. */
export async function renderDocumentPageTile(
  documentId: string,
  tile: DocumentPageTileRequest,
): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("render_document_page_tile", { input: { documentId, ...tile } });
}

/**
 * A PDF page's characters in reading order. `characters` holds one Unicode
 * character per four numbers of `boxes` (`x, y, width, height` in points of a
 * page `width` by `height`); an all-zero box is a space or line break the PDF
 * only implies.
 */
export interface DocumentPageText {
  pageNumber: number;
  width: number;
  height: number;
  characters: string;
  boxes: number[];
}

export async function getDocumentPageText(documentId: string, pageNumber: number): Promise<DocumentPageText> {
  return invoke<DocumentPageText>("get_document_page_text", { input: { documentId, pageNumber } });
}

export interface PreparedAnnotatedPdf {
  preparedExport: PreparedExport;
  /** Annotations drawn into the copy, and the pages they are on. */
  annotationCount: number;
  pageCount: number;
}

/**
 * Prepares a copy of a PDF document with its annotations drawn into the pages,
 * for the learner to save or share. The stored document is not changed.
 */
export async function prepareAnnotatedPdf(
  documentId: string,
  operationId = createOperationId(),
): Promise<PreparedAnnotatedPdf> {
  return invoke<PreparedAnnotatedPdf>("prepare_annotated_pdf", { input: { documentId, operationId } });
}

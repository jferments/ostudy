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
}

export interface HighlightShape {
  kind: "highlight";
  /** `x, y` pairs. */
  points: number[];
  color: string;
  strokeWidth: number;
}

/** A line, or an arrow whose head is at the second point. */
export interface SegmentShape {
  kind: "line" | "arrow";
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color: string;
  strokeWidth: number;
}

/** The outline of a rectangle, or of the ellipse inside it. */
export interface FrameShape {
  kind: "rectangle" | "ellipse";
  x: number;
  y: number;
  width: number;
  height: number;
  color: string;
  strokeWidth: number;
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
}

export type AnnotationShape = InkShape | HighlightShape | SegmentShape | FrameShape | TextShape;
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

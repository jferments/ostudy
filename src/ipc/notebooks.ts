import { createOperationId } from "./common";
import type { Asset } from "./assets";
import type { AnnotationShape } from "./documentAnnotations";
import type { PreparedExport } from "./externalFiles";
import { invoke } from "./transport";

/** The paper a notebook page is drawn on. */
export type Paper = "blank" | "lined" | "grid" | "dots";
/** A new page's size: US Letter, A4, or a wide landscape page for mind maps and diagrams. */
export type PageSize = "letter" | "a4" | "wide";

export interface Notebook {
  id: string;
  /** The subject it is filed in; null is No subject. */
  subjectId: string | null;
  title: string;
  /** What new pages default to. */
  paper: Paper;
  pageSize: PageSize;
  revision: number;
  createdAt: number;
  updatedAt: number;
}

export interface NotebookPage {
  id: string;
  /** From 1, in page order. */
  position: number;
  paper: Paper;
  /** The page size in points. */
  width: number;
  height: number;
}

export interface NotebookItem {
  id: string;
  pageId: string;
  shape: AnnotationShape;
  revision: number;
  createdAt: number;
  updatedAt: number;
}

/** One notebook in the list, with its first page's marks for a small preview. */
export interface NotebookSummary {
  notebook: Notebook;
  pageCount: number;
  linkCount: number;
  firstPage: NotebookPage | null;
  preview: NotebookItem[];
}

export interface NotebookContents {
  notebook: Notebook;
  pages: NotebookPage[];
  /** Earlier marks first on each page. */
  items: NotebookItem[];
  /** Links whose document, page, guide, deck or notebook page is gone. */
  unavailableLinkIds: string[];
  /** The titles of the documents the notebook's clips came from, by document; one that is gone is left out. */
  clipDocuments: Record<string, string>;
}

/**
 * The new state of one mark. A put creates it or replaces all of it, and may
 * move it to another page of the same notebook; deleting one that is already
 * gone succeeds, so a request can be repeated.
 */
export type NotebookItemChange =
  /** A new picture names the image draft its asset was added to, until a save keeps it. */
  | { op: "put"; id: string; pageId: string; shape: AnnotationShape; assetDraftId?: string }
  | { op: "delete"; id: string };

export interface AppliedNotebookItemChanges {
  items: NotebookItem[];
  deletedIds: string[];
}

export async function listNotebooks(): Promise<NotebookSummary[]> {
  return invoke<NotebookSummary[]>("list_notebooks");
}

export async function createNotebook(input: {
  title: string;
  subjectId: string | null;
  paper: Paper;
  pageSize: PageSize;
}): Promise<NotebookContents> {
  return invoke<NotebookContents>("create_notebook", { input });
}

export async function getNotebook(notebookId: string): Promise<NotebookContents> {
  return invoke<NotebookContents>("get_notebook", { input: { notebookId } });
}

export async function renameNotebook(notebookId: string, title: string): Promise<Notebook> {
  return invoke<Notebook>("rename_notebook", { input: { notebookId, title } });
}

export async function moveNotebookToSubject(notebookId: string, subjectId: string | null): Promise<Notebook> {
  return invoke<Notebook>("move_notebook_to_subject", { input: { notebookId, subjectId } });
}

export async function deleteNotebook(notebookId: string): Promise<void> {
  return invoke<void>("delete_notebook", { input: { notebookId } });
}

/** Adds a page at `position` (from 1; past the end adds it last), with the notebook's paper unless given. */
export async function addNotebookPage(
  notebookId: string,
  position: number,
  paper?: Paper,
): Promise<NotebookContents> {
  return invoke<NotebookContents>("add_notebook_page", {
    input: { notebookId, position, ...(paper ? { paper } : {}) },
  });
}

/** Deletes a page and its marks. A notebook keeps at least one page. */
export async function deleteNotebookPage(notebookId: string, pageId: string): Promise<NotebookContents> {
  return invoke<NotebookContents>("delete_notebook_page", { input: { notebookId, pageId } });
}

export async function moveNotebookPage(
  notebookId: string,
  pageId: string,
  position: number,
): Promise<NotebookContents> {
  return invoke<NotebookContents>("move_notebook_page", { input: { notebookId, pageId, position } });
}

export async function applyNotebookItemChanges(
  notebookId: string,
  changes: NotebookItemChange[],
): Promise<AppliedNotebookItemChanges> {
  return invoke<AppliedNotebookItemChanges>("apply_notebook_item_changes", {
    input: { notebookId, changes },
  });
}

export interface PreparedNotebookPdf {
  preparedExport: PreparedExport;
  pageCount: number;
  /** Pictures whose image could not be read, and so were left out. */
  missingPictureCount: number;
}

/**
 * Prepares a PDF of a notebook: its pages with their paper, marks and
 * pictures, and links to its own pages as PDF links.
 */
export async function prepareNotebookPdf(
  notebookId: string,
  operationId = createOperationId(),
): Promise<PreparedNotebookPdf> {
  return invoke<PreparedNotebookPdf>("prepare_notebook_pdf", { input: { notebookId, operationId } });
}

/** A region of a PDF page, in points from its top-left corner, and the page's size. */
export interface DocumentClipRegion {
  documentId: string;
  pageNumber: number;
  pageWidth: number;
  pageHeight: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A clip drawn from its page and staged as a picture, and its size in points. */
export interface StagedDocumentClip {
  asset: Asset;
  width: number;
  height: number;
}

/** Draws a region of a PDF page as a picture, staged in a notebook's image draft. */
export async function stageDocumentClip(draftId: string, region: DocumentClipRegion): Promise<StagedDocumentClip> {
  return invoke<StagedDocumentClip>("stage_document_clip", { input: { draftId, ...region } });
}

/** A notebook holding a clip of a page of a document, and the notebook page it is on. */
export interface DocumentClip {
  pageNumber: number;
  notebookId: string;
  notebookTitle: string;
  notebookPageId: string;
}

/** The notebooks a document's pages were clipped into, page by page. */
export async function listDocumentClips(documentId: string): Promise<DocumentClip[]> {
  return invoke<DocumentClip[]>("list_document_clips", { documentId });
}

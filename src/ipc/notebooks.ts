import { createOperationId } from "./common";
import type { Asset } from "./assets";
import type { AnnotationShape } from "./documentAnnotations";
import type { PreparedExport } from "./externalFiles";
import { invoke } from "./transport";

/** The paper a notebook page is drawn on. */
export type Paper = "blank" | "lined" | "grid" | "dots";
/** A new page's size: US Letter, A4, or a wide landscape page for mind maps and diagrams. */
export type PageSize = "letter" | "a4" | "wide";
/** A page is a sheet of paper; a board is one very large sheet learners pan and zoom around. */
export type PageLayout = "page" | "board";
/** How a board goes into a saved PDF: one page cut to its marks, or split across printable pages. */
export type BoardPrint = "onePage" | { split: PageSize };

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
  layout: PageLayout;
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
  /** Its pages of paper; boards are counted apart. */
  pageCount: number;
  boardCount: number;
  linkCount: number;
  /** The first few links' labels, in page order, each once. */
  linkLabels: string[];
  /** Its Notes folder; null is the top of Notes. */
  folderId: string | null;
  /** When it was pinned; null when it is not. */
  pinnedAt: number | null;
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
  /** A board notebook starts with a board instead of a page. */
  layout?: PageLayout;
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

/** A folder in Notes. Folders may hold folders, a few levels deep. */
export interface NoteFolder {
  id: string;
  /** The folder it is inside; null is the top of Notes. */
  parentId: string | null;
  name: string;
  sortOrder: number;
}

/** Every Notes folder, each folder after the one it is inside. */
export async function listNoteFolders(): Promise<NoteFolder[]> {
  return invoke<NoteFolder[]>("list_note_folders");
}

export async function createNoteFolder(name: string, parentId: string | null = null): Promise<NoteFolder> {
  return invoke<NoteFolder>("create_note_folder", { input: { name, parentId } });
}

export async function renameNoteFolder(folderId: string, name: string): Promise<NoteFolder> {
  return invoke<NoteFolder>("rename_note_folder", { input: { folderId, name } });
}

/** Deletes the folder and the folders inside it; their notebooks go back to the top. */
export async function deleteNoteFolder(folderId: string): Promise<void> {
  return invoke<void>("delete_note_folder", { input: { folderId } });
}

/** Puts a notebook in a folder, or back at the top with null. */
export async function putNotebookInFolder(notebookId: string, folderId: string | null): Promise<void> {
  return invoke<void>("put_notebook_in_folder", { input: { notebookId, folderId } });
}

export async function pinNotebook(notebookId: string, pinned: boolean): Promise<void> {
  return invoke<void>("pin_notebook", { input: { notebookId, pinned } });
}

/**
 * Adds a page at `position` (from 1; past the end adds it last), with the
 * notebook's paper unless given, or a board with `layout: "board"`.
 */
export async function addNotebookPage(
  notebookId: string,
  position: number,
  paper?: Paper,
  layout?: PageLayout,
): Promise<NotebookContents> {
  return invoke<NotebookContents>("add_notebook_page", {
    input: { notebookId, position, ...(paper ? { paper } : {}), ...(layout === "board" ? { layout } : {}) },
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
  boards?: BoardPrint,
): Promise<PreparedNotebookPdf> {
  return invoke<PreparedNotebookPdf>("prepare_notebook_pdf", {
    input: { notebookId, operationId, ...(boards ? { boards } : {}) },
  });
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

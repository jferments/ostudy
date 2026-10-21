import type { AnnotationShape } from "./documentAnnotations";
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
}

/**
 * The new state of one mark. A put creates it or replaces all of it, and may
 * move it to another page of the same notebook; deleting one that is already
 * gone succeeds, so a request can be repeated.
 */
export type NotebookItemChange =
  | { op: "put"; id: string; pageId: string; shape: AnnotationShape }
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

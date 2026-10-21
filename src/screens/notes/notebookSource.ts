import type { AnnotatedPage, DocumentAnnotation } from "../../ipc/documentAnnotations";
import {
  applyNotebookItemChanges,
  getNotebook,
  type NotebookContents,
  type NotebookItemChange,
} from "../../ipc/notebooks";
import type { AnnotatorSource } from "../documents/annotator/useDocumentAnnotator";

/**
 * A notebook as the annotator's pages and marks: page N is the notebook's Nth
 * page, drawn as paper. A notebook's pages are added, moved and deleted between
 * annotator sessions, so the page each number names is fixed while one runs.
 */
export function notebookAnnotations(contents: NotebookContents): { pages: AnnotatedPage[]; annotations: DocumentAnnotation[] } {
  const numbers = new Map(contents.pages.map((page, index) => [page.id, index + 1]));
  return {
    pages: contents.pages.map((page, index) => ({
      pageNumber: index + 1,
      assetId: "",
      width: page.width,
      height: page.height,
      imageAvailable: false,
    })),
    annotations: contents.items.flatMap((item) => {
      const pageNumber = numbers.get(item.pageId);
      return pageNumber === undefined ? [] : [{
        id: item.id,
        pageNumber,
        shape: item.shape,
        revision: item.revision,
        createdAt: item.createdAt,
        updatedAt: item.updatedAt,
      }];
    }),
  };
}

/** Reads and saves a notebook for the annotator, reporting what each read found. */
export function notebookSource(onLoaded: (contents: NotebookContents) => void): AnnotatorSource {
  let pageIds: readonly string[] = [];
  return {
    async load(notebookId) {
      const contents = await getNotebook(notebookId);
      pageIds = contents.pages.map((page) => page.id);
      onLoaded(contents);
      return notebookAnnotations(contents);
    },
    async send(notebookId, changes) {
      const sent = changes.map((change): NotebookItemChange => {
        if (change.op === "delete") return change;
        const pageId = pageIds[change.pageNumber - 1];
        if (!pageId) throw new Error("That page is no longer in this notebook.");
        return { op: "put", id: change.id, pageId, shape: change.shape };
      });
      return applyNotebookItemChanges(notebookId, sent);
    },
  };
}

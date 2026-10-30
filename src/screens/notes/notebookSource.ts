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

/**
 * Pictures added since their notebook last saved, by asset, with the draft that
 * staged each one. The save that first stores a picture keeps its image; until
 * then the image is only staged and goes if the picture is never saved.
 */
const pictureDrafts = new Map<string, string>();

/** Notes that `assetId` was staged by `draftId` for a picture not saved yet. */
export function rememberPictureDraft(assetId: string, draftId: string): void {
  pictureDrafts.set(assetId, draftId);
}

/** Reads and saves a notebook for the annotator, reporting what each read found. */
export function notebookSource(onLoaded: (contents: NotebookContents) => void): AnnotatorSource {
  let pageIds: readonly string[] = [];
  return {
    notebook: true,
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
        const assetDraftId = change.shape.kind === "image" ? pictureDrafts.get(change.shape.assetId) : undefined;
        return { op: "put", id: change.id, pageId, shape: change.shape, ...(assetDraftId ? { assetDraftId } : {}) };
      });
      const result = await applyNotebookItemChanges(notebookId, sent);
      for (const change of sent) {
        if (change.op === "put" && change.assetDraftId && change.shape.kind === "image") pictureDrafts.delete(change.shape.assetId);
      }
      return result;
    },
  };
}

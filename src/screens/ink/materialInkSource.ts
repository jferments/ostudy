import type { AnnotatedPage, DocumentAnnotation } from "../../ipc/documentAnnotations";
import { applyMaterialInkChanges, getMaterialInk, type InkChange, type InkMark, type InkMaterial } from "../../ipc/materialInk";
import type { AnnotatorSource } from "../documents/annotator/useDocumentAnnotator";

/** A designed page to write on: its key, its size in points, and its picture. */
export interface InkPage {
  key: string;
  width: number;
  height: number;
  image: string;
}

/** The marks on `pages` as the annotator's pages and annotations: page N is `pages[N - 1]`. */
export function inkAnnotations(pages: readonly InkPage[], marks: readonly InkMark[]): {
  pages: AnnotatedPage[];
  annotations: DocumentAnnotation[];
} {
  const numbers = new Map(pages.map((page, index) => [page.key, index + 1]));
  return {
    pages: pages.map((page, index) => ({
      pageNumber: index + 1,
      assetId: "",
      width: page.width,
      height: page.height,
      imageAvailable: false,
    })),
    annotations: marks.flatMap((mark) => {
      const pageNumber = numbers.get(mark.pageKey);
      return pageNumber === undefined ? [] : [{
        id: mark.id,
        pageNumber,
        shape: mark.shape,
        revision: mark.revision,
        createdAt: mark.createdAt,
        updatedAt: mark.updatedAt,
      }];
    }),
  };
}

/** Reads and saves a material's marks for the annotator, on the pages it was opened with. */
export function materialInkSource(material: InkMaterial, pages: readonly InkPage[]): AnnotatorSource {
  return {
    async load() {
      return inkAnnotations(pages, await getMaterialInk(material));
    },
    async send(_key, changes) {
      const sent = changes.map((change): InkChange => {
        if (change.op === "delete") return change;
        const page = pages[change.pageNumber - 1];
        if (!page) throw new Error("That page is no longer here.");
        return { op: "put", id: change.id, pageKey: page.key, shape: change.shape };
      });
      return applyMaterialInkChanges(material, sent);
    },
  };
}

/** The key the annotator's save queue and undo history are kept under. */
export function inkMaterialKey(material: InkMaterial): string {
  return material.kind === "lesson" ? `lesson:${material.lessonId}` : `guide:${material.documentId}`;
}

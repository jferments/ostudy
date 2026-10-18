import type { DocumentSummary } from "../../../ipc/documents";

/** Annotation draws over the page images a PDF import renders, once they exist. */
export function canAnnotateDocument(document: DocumentSummary): boolean {
  return document.sourceFormat === "pdf" && document.renderedPageCount > 0 && document.lifecycleState !== "processing";
}

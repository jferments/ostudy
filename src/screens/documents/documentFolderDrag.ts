export const DOCUMENT_FOLDER_DRAG_TYPE = "application/x-ostudy-document-id";

export function beginDocumentFolderDrag(dataTransfer: DataTransfer, documentId: string): void {
  dataTransfer.effectAllowed = "copy";
  dataTransfer.setData(DOCUMENT_FOLDER_DRAG_TYPE, documentId);
}

export function hasDocumentFolderDrag(dataTransfer: DataTransfer): boolean {
  return Array.from(dataTransfer.types).includes(DOCUMENT_FOLDER_DRAG_TYPE);
}

export function readDocumentFolderDrag(dataTransfer: DataTransfer): string | null {
  if (!hasDocumentFolderDrag(dataTransfer)) return null;
  const value = dataTransfer.getData(DOCUMENT_FOLDER_DRAG_TYPE).trim();
  return value.length === 32 ? value : null;
}

export function isInteractiveDocumentDragOrigin(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return target.closest("button,a,input,select,textarea,[contenteditable='true']") !== null;
}

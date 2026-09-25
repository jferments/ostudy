import { describe, expect, it } from "vitest";

import {
  beginDocumentFolderDrag,
  DOCUMENT_FOLDER_DRAG_TYPE,
  hasDocumentFolderDrag,
  readDocumentFolderDrag,
} from "./documentFolderDrag";

function transfer() {
  const values = new Map<string, string>();
  return {
    effectAllowed: "none",
    dropEffect: "none",
    get types() { return Array.from(values.keys()); },
    setData(type: string, value: string) { values.set(type, value); },
    getData(type: string) { return values.get(type) ?? ""; },
  } as unknown as DataTransfer;
}

describe("document folder drag transport", () => {
  it("uses an application-owned copy payload and rejects unrelated or malformed drops", () => {
    const data = transfer();
    const documentId = "a".repeat(32);
    expect(hasDocumentFolderDrag(data)).toBe(false);
    expect(readDocumentFolderDrag(data)).toBeNull();

    beginDocumentFolderDrag(data, documentId);
    expect(data.effectAllowed).toBe("copy");
    expect(hasDocumentFolderDrag(data)).toBe(true);
    expect(data.getData(DOCUMENT_FOLDER_DRAG_TYPE)).toBe(documentId);
    expect(readDocumentFolderDrag(data)).toBe(documentId);

    data.setData(DOCUMENT_FOLDER_DRAG_TYPE, "bad");
    expect(readDocumentFolderDrag(data)).toBeNull();
  });
});

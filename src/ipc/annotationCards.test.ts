import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getDocumentMarkedCards, linkCardNoteToAnnotation } from "./annotationCards";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

const noteId = "a".repeat(32);
const annotationId = "b".repeat(32);
const documentId = "c".repeat(32);

describe("cards from marks IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({});
  });

  it("links a saved card to the mark it was made from", async () => {
    await linkCardNoteToAnnotation(noteId, annotationId);
    expect(invoke).toHaveBeenCalledWith("link_card_note_to_annotation", {
      input: { noteId, annotationId },
    });
  });

  it("reads the cards made from a document's marks", async () => {
    await getDocumentMarkedCards(documentId);
    expect(invoke).toHaveBeenCalledWith("get_document_marked_cards", {
      input: { documentId },
    });
  });
});

import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  createSubject,
  deleteSubject,
  dismissSubjectFilingReview,
  getSubjectFilingReviews,
  getSubjectLibrary,
  moveDeckToSubject,
  moveDocumentToSubject,
  moveSubject,
  renameSubject,
  setDocumentStudyRanges,
  setSubjectLeftOutDocuments,
} from "./subjects";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

const library = {
  libraryRevision: 1,
  documentFilingRevision: 1,
  subjects: [],
  decks: [],
  documents: [],
  studyRanges: [],
  checkTheseCount: 0,
};

describe("subject IPC", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue(library);
  });

  it("keeps subject structure and filing behind typed native commands", async () => {
    await getSubjectLibrary();
    expect(invoke).toHaveBeenLastCalledWith("get_subject_library");

    const create = { name: "Pharmacology", expectedLibraryRevision: 1 };
    await createSubject(create);
    expect(invoke).toHaveBeenLastCalledWith("create_subject", { input: create });

    const rename = {
      subjectId: "a".repeat(32),
      name: "Clinical pharmacology",
      revision: 1,
      expectedLibraryRevision: 2,
    };
    await renameSubject(rename);
    expect(invoke).toHaveBeenLastCalledWith("rename_subject", { input: rename });

    const move = { subjectId: "a".repeat(32), targetIndex: 0, expectedLibraryRevision: 3 };
    await moveSubject(move);
    expect(invoke).toHaveBeenLastCalledWith("move_subject", { input: move });

    const remove = { subjectId: "a".repeat(32), revision: 2, expectedLibraryRevision: 4 };
    await deleteSubject(remove);
    expect(invoke).toHaveBeenLastCalledWith("delete_subject", { input: remove });

    const deck = { deckId: "d".repeat(32), subjectId: null, expectedLibraryRevision: 5 };
    await moveDeckToSubject(deck);
    expect(invoke).toHaveBeenLastCalledWith("move_deck_to_subject", { input: deck });

    const document = {
      documentId: "c".repeat(32),
      subjectId: "a".repeat(32),
      expectedDocumentFilingRevision: 1,
    };
    await moveDocumentToSubject(document);
    expect(invoke).toHaveBeenLastCalledWith("move_document_to_subject", { input: document });

    const ranges = {
      documentId: "c".repeat(32),
      ranges: [{ unitKind: "page", startUnit: 1, endUnit: 4 }],
      expectedDocumentFilingRevision: 2,
    };
    await setDocumentStudyRanges(ranges);
    expect(invoke).toHaveBeenLastCalledWith("set_document_study_ranges", { input: ranges });

    const leftOut = { subjectId: "a".repeat(32), documentIds: ["c".repeat(32)], expectedDocumentFilingRevision: 3 };
    await setSubjectLeftOutDocuments(leftOut);
    expect(invoke).toHaveBeenLastCalledWith("set_subject_left_out_documents", { input: leftOut });
  });

  it("reads and dismisses the Check these list", async () => {
    vi.mocked(invoke).mockResolvedValue([]);
    await expect(getSubjectFilingReviews()).resolves.toEqual([]);
    expect(invoke).toHaveBeenLastCalledWith("get_subject_filing_reviews");

    const dismissal = { entityKind: "deck" as const, entityId: "d".repeat(32) };
    await dismissSubjectFilingReview(dismissal);
    expect(invoke).toHaveBeenLastCalledWith("dismiss_subject_filing_review", {
      input: dismissal,
    });
  });
});

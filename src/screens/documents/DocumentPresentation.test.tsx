import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as collections from "../../ipc/collections";
import * as documentIpc from "../../ipc/documents";
import * as studyGuideIpc from "../../ipc/studyGuides";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { DocumentsScreen } from "../DocumentsScreen";

vi.mock("../../platform/files", () => ({ selectExternalImport: vi.fn() }));
vi.mock("../../ipc/documents", () => ({
  listDocuments: vi.fn(), getDocumentGenerationProgress: vi.fn(),
  importDocument: vi.fn(), startDocumentFlashcardGeneration: vi.fn(),
  retryDocumentFlashcardGeneration: vi.fn(),
  generateMoreDocumentFlashcards: vi.fn(), cancelDocumentFlashcardGeneration: vi.fn(),
  openDocumentSource: vi.fn(), deleteDocument: vi.fn(),
}));
vi.mock("../../ipc/studyGuides", () => ({
  generateDocumentStudyGuide: vi.fn(),
  cancelDocumentStudyGuideGeneration: vi.fn(),
  getDocumentLearningPlan: vi.fn().mockResolvedValue(null),
}));
vi.mock("../../ipc/collections", () => ({
  getDocumentCollectionOrganization: vi.fn().mockResolvedValue({
    revision: 1, collections: [], memberships: [], ranges: [],
  }),
  createDocumentCollection: vi.fn(),
  updateDocumentCollection: vi.fn(),
  deleteDocumentCollection: vi.fn(),
  addDocumentCollectionMembership: vi.fn(),
  moveDocumentCollectionMembership: vi.fn(),
  removeDocumentCollectionMembership: vi.fn(),
  setDocumentCollectionRanges: vi.fn(),
  resolveDocumentSourceSelection: vi.fn(),
}));

const documentId = "a".repeat(32);

function makeDocument(
  overrides: Partial<documentIpc.DocumentSummary> = {},
): documentIpc.DocumentSummary {
  return {
    id: documentId,
    title: "Cardiac physiology",
    originalFilename: "cardiac-physiology.pdf",
    byteSize: 2048,
    sha256: "0".repeat(64),
    lifecycleState: "ready",
    sourceFormat: "pdf",
    sourceUnitKind: "page",
    sourceUnitCount: 10,
    pageCount: 10,
    extractedPageCount: 10,
    renderedPageCount: 10,
    extractionMode: "native",
    ocrPageCount: 0,
    ocrCompletedPageCount: 0,
    ocrMeanConfidence: null,
    ocrEngine: "",
    scaleMode: "standard",
    estimatedSourceTokens: 0,
    outlineWindowCount: 0,
    outlineCompletedWindowCount: 0,
    generationWindowCount: 0,
    generationCompletedWindowCount: 0,
    generatedNoteCount: 6,
    generatedCardCount: 7,
    generatedDeckId: "b".repeat(32),
    safeErrorCode: "",
    safeErrorMessage: "",
    createdAt: 1_700_000_000_000,
    updatedAt: 1_700_000_000_000,
    completedAt: 1_700_000_000_000,
    revision: 2,
    processingDurationMs: 12_345,
    apiTokenCount: 45_678,
    apiTokenUsageComplete: true,
    ...overrides,
  };
}

function renderDocuments() {
  const showToast = vi.fn();
  render(<OverlayProvider><DocumentsScreen showToast={showToast} refreshDecks={async () => []} /></OverlayProvider>);
  return showToast;
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(collections.getDocumentCollectionOrganization).mockResolvedValue({
    revision: 1, collections: [], memberships: [], ranges: [],
  });
  vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
  vi.mocked(documentIpc.getDocumentGenerationProgress).mockResolvedValue(null);
  vi.mocked(documentIpc.startDocumentFlashcardGeneration).mockResolvedValue(undefined);
  vi.mocked(studyGuideIpc.generateDocumentStudyGuide).mockResolvedValue({ revision: 1 } as studyGuideIpc.DocumentStudyGuide);
  vi.mocked(studyGuideIpc.cancelDocumentStudyGuideGeneration).mockResolvedValue(undefined);
});

describe("Document job presentation", () => {
  it("keeps document actions open while collection discovery settles", async () => {
    let finish!: (value: collections.DocumentCollectionOrganization) => void;
    vi.mocked(collections.getDocumentCollectionOrganization).mockReturnValueOnce(
      new Promise((resolve) => { finish = resolve; }),
    );
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({ generatedDeckId: null, generatedCardCount: 0 }),
    ]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    fireEvent.click(screen.getByRole("button", { name: "More actions for Cardiac physiology" }));
    expect(await screen.findByText("Retry generation")).toBeTruthy();

    await act(async () => {
      finish({ revision: 1, collections: [], memberships: [], ranges: [] });
      await Promise.resolve();
    });
    expect(screen.getByText("Retry generation")).toBeTruthy();
  });

  it("keeps nonfatal analysis details out of cards and successful generation toasts", async () => {
    const before = makeDocument({ generatedDeckId: null, generatedCardCount: 0, analysisPendingItems: 4 });
    const after = makeDocument({ analysisPendingItems: 4 });
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([before]);
    vi.mocked(documentIpc.startDocumentFlashcardGeneration).mockImplementation(async () => {
      vi.mocked(documentIpc.listDocuments).mockResolvedValue([after]);
    });
    const toast = renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    fireEvent.click(screen.getByRole("button", { name: "More actions for Cardiac physiology" }));
    fireEvent.click(await screen.findByText("Retry generation"));

    await waitFor(() => expect(toast).toHaveBeenCalledWith(
      "success",
      "Created 7 flashcards in a new deck.",
    ));
    expect(screen.queryByText(/Analysis is partial|knowledge proposals|source dispositions/iu)).toBeNull();
    expect(JSON.stringify(toast.mock.calls)).not.toMatch(/partial|unresolved/iu);
  });

  it("offers automatic study-guide stop only through the overflow menu", async () => {
    const before = makeDocument({ generatedDeckId: null, generatedCardCount: 0 });
    const after = makeDocument();
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([before]);
    vi.mocked(documentIpc.startDocumentFlashcardGeneration).mockImplementation(async () => {
      vi.mocked(documentIpc.listDocuments).mockResolvedValue([after]);
    });
    vi.mocked(studyGuideIpc.generateDocumentStudyGuide).mockReturnValue(new Promise(() => {}));
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    fireEvent.click(screen.getByRole("button", { name: "More actions for Cardiac physiology" }));
    fireEvent.click(await screen.findByText("Retry generation"));
    await waitFor(() => expect(studyGuideIpc.generateDocumentStudyGuide).toHaveBeenCalledWith(documentId));

    expect(screen.queryByRole("button", { name: /study guide/iu })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "More actions for Cardiac physiology" }));
    fireEvent.click(await screen.findByText("Stop study guide generation"));
    await waitFor(() => expect(
      studyGuideIpc.cancelDocumentStudyGuideGeneration,
    ).toHaveBeenCalledWith(documentId));
  });

  it("keeps automatic guide failures visible without hiding saved flashcards", async () => {
    const before = makeDocument({ generatedDeckId: null, generatedCardCount: 0 });
    const after = makeDocument();
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([before]);
    vi.mocked(documentIpc.startDocumentFlashcardGeneration).mockImplementation(async () => {
      vi.mocked(documentIpc.listDocuments).mockResolvedValue([after]);
    });
    vi.mocked(studyGuideIpc.generateDocumentStudyGuide).mockRejectedValue(
      new Error("The provider could not finish the guide."),
    );
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    fireEvent.click(screen.getByRole("button", { name: "More actions for Cardiac physiology" }));
    fireEvent.click(await screen.findByText("Retry generation"));

    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain(
      "Flashcards were saved, but automatic Study Guide generation failed",
    ));
    expect(screen.getByText("7 cards")).toBeTruthy();
  });
});

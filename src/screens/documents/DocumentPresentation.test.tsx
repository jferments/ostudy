import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
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
  vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
  vi.mocked(documentIpc.getDocumentGenerationProgress).mockResolvedValue(null);
  vi.mocked(studyGuideIpc.cancelDocumentStudyGuideGeneration).mockResolvedValue(undefined);
});

describe("Document job presentation", () => {
  it("keeps nonfatal analysis details out of cards and successful generation toasts", async () => {
    const before = makeDocument({ generatedDeckId: null, analysisPendingItems: 4 });
    const after = makeDocument({ analysisPendingItems: 4 });
    vi.mocked(documentIpc.listDocuments).mockResolvedValueOnce([before]).mockResolvedValue([after]);
    vi.mocked(documentIpc.retryDocumentFlashcardGeneration).mockResolvedValue({
      document: after, deckId: after.generatedDeckId!, createdNoteCount: 6, createdCardCount: 7, skippedDuplicateCount: 0,
    });
    const toast = renderDocuments();
    fireEvent.click(await screen.findByRole("button", { name: "Retry generation" }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("success", "Created 7 flashcards in a new deck."));
    expect(screen.queryByText(/Analysis is partial|knowledge proposals|source dispositions/iu)).toBeNull();
    expect(JSON.stringify(toast.mock.calls)).not.toMatch(/partial|unresolved/iu);
  });

  it("uses the red stop-sign icon in one generating button and cancels there", async () => {
    vi.mocked(studyGuideIpc.generateDocumentStudyGuide).mockReturnValue(new Promise(() => {}));
    renderDocuments();
    fireEvent.click(await screen.findByRole("button", { name: "Generate study guide" }));
    const generating = await screen.findByRole("button", { name: "Generating study guide…" });
    expect(generating.querySelector('[data-tone="danger"] svg')).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Stop study guide$/u })).toBeNull();
    fireEvent.click(generating);
    await waitFor(() => expect(studyGuideIpc.cancelDocumentStudyGuideGeneration).toHaveBeenCalledWith(documentId));
    expect(screen.getByRole("button", { name: "Stopping study guide…" }).hasAttribute("disabled")).toBe(true);
  });

  it("keeps terminal failure explanations visible", async () => {
    vi.mocked(studyGuideIpc.generateDocumentStudyGuide).mockRejectedValue(new Error("The provider could not finish the guide."));
    renderDocuments();
    fireEvent.click(await screen.findByRole("button", { name: "Generate study guide" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("The provider could not finish the guide."));
  });
});

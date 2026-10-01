import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as collectionIpc from "../ipc/collections";
import * as artifactIpc from "../ipc/documentArtifactProgress";
import * as documentIpc from "../ipc/documents";
import { MAX_GENERATION_INSTRUCTIONS_BYTES } from "../ipc/generationInstructions";
import * as guideIpc from "../ipc/studyGuides";
import * as platformFiles from "../platform/files";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { DocumentsScreen } from "./DocumentsScreen";

vi.mock("../platform/files", () => ({ selectExternalImport: vi.fn() }));
vi.mock("../ipc/collections", async (importOriginal) => ({
  ...await importOriginal<typeof import("../ipc/collections")>(),
  getDocumentCollectionOrganization: vi.fn(),
}));
vi.mock("../ipc/documentArtifactProgress", () => ({ listLatestDocumentStudyGuideJobs: vi.fn() }));
vi.mock("../ipc/documents", () => ({
  listDocuments: vi.fn(), getDocumentGenerationProgress: vi.fn(), importDocument: vi.fn(),
  startDocumentFlashcardGeneration: vi.fn(), retryDocumentFlashcardGeneration: vi.fn(),
  generateMoreDocumentFlashcards: vi.fn(), cancelDocumentFlashcardGeneration: vi.fn(),
  openDocumentSource: vi.fn(), deleteDocument: vi.fn(),
}));
vi.mock("../ipc/studyGuides", () => ({
  generateDocumentStudyGuide: vi.fn(), getDocumentStudyGuide: vi.fn(),
  cancelDocumentStudyGuideGeneration: vi.fn(), getDocumentLearningPlan: vi.fn(),
}));

const documentId = "a".repeat(32);
const deckId = "b".repeat(32);
const stagedId = "c".repeat(32);
function document(overrides: Partial<documentIpc.DocumentSummary> = {}): documentIpc.DocumentSummary {
  return {
    id: documentId, title: "Neurological assessments", originalFilename: "assessments.pdf",
    byteSize: 2048, sha256: "0".repeat(64), lifecycleState: "ready",
    sourceFormat: "pdf", sourceUnitKind: "page", sourceUnitCount: 10, pageCount: 10,
    extractedPageCount: 10, renderedPageCount: 10, extractionMode: "native",
    ocrPageCount: 0, ocrCompletedPageCount: 0, ocrMeanConfidence: null, ocrEngine: "",
    scaleMode: "standard", estimatedSourceTokens: 0, outlineWindowCount: 0,
    outlineCompletedWindowCount: 0, generationWindowCount: 0, generationCompletedWindowCount: 0,
    generatedNoteCount: 6, generatedCardCount: 7, generatedDeckId: deckId,
    safeErrorCode: "", safeErrorMessage: "", createdAt: 1_700_000_000_000,
    updatedAt: 1_700_000_000_000, completedAt: 1_700_000_000_000, revision: 2,
    processingDurationMs: 12345, apiTokenCount: 45678, apiTokenUsageComplete: true,
    ...overrides,
  };
}
function renderDocuments() {
  return render(<OverlayProvider><DocumentsScreen
    refreshDecks={vi.fn().mockResolvedValue([])} showToast={vi.fn()} />
  </OverlayProvider>);
}
function editInstructions(label: string, value: string, root: HTMLElement = documentBody()): void {
  const queries = within(root);
  const summary = queries.getByText(label, { selector: "summary" });
  if (!summary.parentElement?.hasAttribute("open")) fireEvent.click(summary);
  fireEvent.change(queries.getByLabelText(label), { target: { value } });
}
function documentBody(): HTMLElement { return globalThis.document.body; }
async function selectGenerateMore(title = "Neurological assessments") {
  const label = `More actions for ${title}`;
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: label }));
    await Promise.resolve();
  });
  const menu = await screen.findByRole("menu", { name: label });
  const action = within(menu).getByRole("menuitem", { name: "Generate more cards" });
  await waitFor(() => expect(action).not.toHaveAttribute("aria-disabled", "true"));
  await act(async () => { fireEvent.click(action); });
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(collectionIpc.getDocumentCollectionOrganization).mockResolvedValue({
    revision: 1, collections: [], memberships: [], ranges: [],
  });
  vi.mocked(artifactIpc.listLatestDocumentStudyGuideJobs).mockResolvedValue([]);
  vi.mocked(documentIpc.listDocuments).mockResolvedValue([]);
  vi.mocked(documentIpc.getDocumentGenerationProgress).mockResolvedValue(null);
  vi.mocked(documentIpc.startDocumentFlashcardGeneration).mockResolvedValue(undefined);
  vi.mocked(documentIpc.importDocument).mockResolvedValue({ documentId });
  vi.mocked(platformFiles.selectExternalImport).mockResolvedValue(null);
  vi.mocked(guideIpc.getDocumentLearningPlan).mockResolvedValue(null);
  vi.mocked(guideIpc.getDocumentStudyGuide).mockResolvedValue({ revision: 1 } as guideIpc.DocumentStudyGuide);
  vi.mocked(guideIpc.generateDocumentStudyGuide).mockResolvedValue({ revision: 2 } as guideIpc.DocumentStudyGuide);
  vi.mocked(documentIpc.generateMoreDocumentFlashcards).mockResolvedValue({
    documentId, deckId, createdNoteCount: 2, createdCardCount: 2, skippedDuplicateCount: 0,
  });
});

describe("Documents guidance through the application controller", () => {
  it("passes a learner-authored upload prompt into the real initial-generation gateway", async () => {
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({ id: stagedId } as
      NonNullable<Awaited<ReturnType<typeof platformFiles.selectExternalImport>>>);
    renderDocuments();
    await screen.findByRole("heading", { name: "No documents yet" });
    editInstructions("Instructions for new document flashcards", "  Nursing cranial nerve assessments  ");
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Upload document" })); });
    await waitFor(() => expect(documentIpc.startDocumentFlashcardGeneration)
      .toHaveBeenCalledWith(documentId, "Nursing cranial nerve assessments"));
    expect(documentIpc.importDocument).toHaveBeenCalledExactlyOnceWith(stagedId);
    expect(platformFiles.selectExternalImport).toHaveBeenCalledExactlyOnceWith("document");
    expect(documentIpc.generateMoreDocumentFlashcards).not.toHaveBeenCalled();
  });

  it("keeps a document's guidance independent of upload defaults and supports explicitly clearing it", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([document()]);
    renderDocuments();
    await screen.findByRole("heading", { name: "Neurological assessments" });
    editInstructions("Instructions for new document flashcards", "Future uploads only");
    const article = screen.getByRole("article");
    editInstructions("Flashcard instructions (optional)", "Ask how each nerve is assessed", article);
    await selectGenerateMore();
    await waitFor(() => expect(documentIpc.generateMoreDocumentFlashcards)
      .toHaveBeenLastCalledWith(documentId, "Ask how each nerve is assessed"));
    await waitFor(() => expect(within(article).getByLabelText("Flashcard instructions (optional)")).not.toBeDisabled());
    editInstructions("Flashcard instructions (optional)", "", article);
    await selectGenerateMore();
    await waitFor(() => expect(documentIpc.generateMoreDocumentFlashcards)
      .toHaveBeenLastCalledWith(documentId, undefined));
    expect(documentIpc.generateMoreDocumentFlashcards).toHaveBeenCalledTimes(2);
    expect(documentIpc.importDocument).not.toHaveBeenCalled();
    expect(documentIpc.startDocumentFlashcardGeneration).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Instructions for new document flashcards")).toHaveValue("Future uploads only");
  });

  it("shows validation before opening a picker or starting any native mutation", async () => {
    renderDocuments();
    await screen.findByRole("heading", { name: "No documents yet" });
    editInstructions("Instructions for new document flashcards", "x".repeat(MAX_GENERATION_INSTRUCTIONS_BYTES + 1));
    expect(screen.getByLabelText("Instructions for new document flashcards")).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByRole("button", { name: "Upload document" })).toBeDisabled();
    expect(screen.getByText(/Shorten them before starting; they have not been truncated/)).toBeTruthy();
    expect(platformFiles.selectExternalImport).not.toHaveBeenCalled();
    expect(documentIpc.importDocument).not.toHaveBeenCalled();
    expect(documentIpc.startDocumentFlashcardGeneration).not.toHaveBeenCalled();
  });
});

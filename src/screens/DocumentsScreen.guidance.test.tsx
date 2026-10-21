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
async function openMakeMoreCards(title = "Neurological assessments"): Promise<HTMLElement> {
  const label = `More actions for ${title}`;
  const trigger = screen.getByRole("button", { name: label });
  // Base UI wires the trigger in a passive effect, so an early click can be dropped.
  for (let attempt = 0; attempt < 3 && trigger.getAttribute("aria-expanded") !== "true"; attempt += 1) {
    await act(async () => { fireEvent.click(trigger); });
  }
  const menu = await screen.findByRole("menu", { name: label });
  const action = within(menu).getByRole("menuitem", { name: "Make more cards…" });
  await waitFor(() => expect(action.getAttribute("aria-disabled")).not.toBe("true"));
  await act(async () => { fireEvent.click(action); });
  return screen.findByRole("dialog", { name: "Make more cards" });
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
  it("uploads without asking for flashcard instructions", async () => {
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({ id: stagedId } as
      NonNullable<Awaited<ReturnType<typeof platformFiles.selectExternalImport>>>);
    renderDocuments();
    await screen.findByRole("heading", { name: "No documents yet" });
    expect(screen.queryByRole("textbox")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Upload" }));
    await act(async () => { fireEvent.click(await screen.findByRole("menuitem", { name: "Document…" })); });
    await waitFor(() => expect(documentIpc.startDocumentFlashcardGeneration)
      .toHaveBeenCalledWith(documentId, undefined));
    expect(documentIpc.importDocument).toHaveBeenCalledTimes(1);
    expect(documentIpc.importDocument).toHaveBeenCalledWith(stagedId, {});
    expect(platformFiles.selectExternalImport).toHaveBeenCalledTimes(1);
    expect(platformFiles.selectExternalImport).toHaveBeenCalledWith("document");
    expect(documentIpc.generateMoreDocumentFlashcards).not.toHaveBeenCalled();
  });

  it("takes a document's instructions when making more cards and supports explicitly clearing them", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([document()]);
    renderDocuments();
    await screen.findByRole("heading", { name: "Neurological assessments" });
    let dialog = await openMakeMoreCards();
    fireEvent.change(within(dialog).getByLabelText("Instructions (optional)"), {
      target: { value: "Ask how each nerve is assessed" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Make cards" }));
    await waitFor(() => expect(documentIpc.generateMoreDocumentFlashcards)
      .toHaveBeenLastCalledWith(documentId, "Ask how each nerve is assessed"));

    dialog = await openMakeMoreCards();
    const field = within(dialog).getByLabelText("Instructions (optional)") as HTMLTextAreaElement;
    expect(field.value).toBe("Ask how each nerve is assessed");
    fireEvent.change(field, { target: { value: "" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Make cards" }));
    await waitFor(() => expect(documentIpc.generateMoreDocumentFlashcards)
      .toHaveBeenLastCalledWith(documentId, undefined));
    expect(documentIpc.generateMoreDocumentFlashcards).toHaveBeenCalledTimes(2);
    expect(documentIpc.importDocument).not.toHaveBeenCalled();
    expect(documentIpc.startDocumentFlashcardGeneration).not.toHaveBeenCalled();
  });

  it("shows validation before starting any native mutation", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([document()]);
    renderDocuments();
    await screen.findByRole("heading", { name: "Neurological assessments" });
    const dialog = await openMakeMoreCards();
    const field = within(dialog).getByLabelText("Instructions (optional)");
    fireEvent.change(field, { target: { value: "x".repeat(MAX_GENERATION_INSTRUCTIONS_BYTES + 1) } });
    expect(field.getAttribute("aria-invalid")).toBe("true");
    expect((within(dialog).getByRole("button", { name: "Make cards" }) as HTMLButtonElement).disabled).toBe(true);
    expect(within(dialog).getByText(/Shorten them before starting; they have not been truncated/)).toBeTruthy();
    expect(documentIpc.generateMoreDocumentFlashcards).not.toHaveBeenCalled();
  });
});

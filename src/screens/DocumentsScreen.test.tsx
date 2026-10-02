import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as collectionIpc from "../ipc/collections";
import * as artifactIpc from "../ipc/documentArtifactProgress";
import * as documentIpc from "../ipc/documents";
import * as studyGuideIpc from "../ipc/studyGuides";
import * as platformFiles from "../platform/files";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { DOCUMENT_FOLDER_DRAG_TYPE } from "./documents/documentFolderDrag";
import { DocumentsScreen } from "./DocumentsScreen";

vi.mock("../platform/files", () => ({ selectExternalImport: vi.fn() }));
vi.mock("../ipc/collections", async (importOriginal) => ({
  ...await importOriginal<typeof import("../ipc/collections")>(),
  getDocumentCollectionOrganization: vi.fn(),
}));
vi.mock("../ipc/documentArtifactProgress", () => ({
  listLatestDocumentStudyGuideJobs: vi.fn(),
}));
vi.mock("../ipc/documents", () => ({
  listDocuments: vi.fn(),
  getDocumentGenerationProgress: vi.fn(),
  importDocument: vi.fn(),
  startDocumentFlashcardGeneration: vi.fn(),
  retryDocumentFlashcardGeneration: vi.fn(),
  generateMoreDocumentFlashcards: vi.fn(),
  cancelDocumentFlashcardGeneration: vi.fn(),
  openDocumentSource: vi.fn(),
  deleteDocument: vi.fn(),
}));
vi.mock("../ipc/studyGuides", () => ({
  generateDocumentStudyGuide: vi.fn(),
  getDocumentStudyGuide: vi.fn(),
  cancelDocumentStudyGuideGeneration: vi.fn(),
  getDocumentLearningPlan: vi.fn(),
}));
vi.mock("../ipc/common", async (importOriginal) => {
  const original = await importOriginal<typeof import("../ipc/common")>();
  return {
    ...original,
    getErrorCode: (error: unknown) =>
      error && typeof error === "object" && "code" in error ? String(error.code) : "unknown",
    getErrorMessage: (error: unknown) =>
      error instanceof Error ? error.message : "Unexpected document error",
  };
});

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

function makeProgress(phase: string): documentIpc.DocumentGenerationProgress {
  return {
    startedAt: Date.now() - 1_000,
    phase,
    phaseStartedAt: Date.now() - 500,
    currentWindow: null,
    currentWindowEnd: null,
    completedWindows: 0,
    totalWindows: 0,
    currentPageStart: null,
    currentPageEnd: null,
    currentPartition: null,
    providerKind: null,
    modelId: null,
    requestedReasoningEffort: null,
    reasoningEffort: "none",
    currentRequestStartedAt: null,
    currentRequestStage: null,
    currentRequestBytes: null,
    currentRequestTextBytes: null,
    currentRequestImageCount: null,
    currentRequestEvidenceBytes: null,
    currentRequestMaxOutputTokens: null,
    providerRequestCount: 0,
    retryCount: 0,
    promptTokens: 0,
    completionTokens: 0,
    reasoningTokens: 0,
    totalTokens: 0,
    cachedPromptTokens: 0,
    cacheWriteTokens: 0,
    usageReportedRequests: 0,
    usageUnreportedRequests: 0,
    providerActivity: "preparing",
    lastProviderActivityAt: null,
    lastModelActivityAt: null,
  };
}

function renderDocuments(showToast = vi.fn(), refreshDecks = vi.fn().mockResolvedValue([])) {
  render(
    <OverlayProvider>
      <DocumentsScreen refreshDecks={refreshDecks} showToast={showToast} />
    </OverlayProvider>,
  );
  return { showToast, refreshDecks };
}

async function openDocumentActions(title = "Cardiac physiology"): Promise<HTMLElement> {
  const label = `More actions for ${title}`;
  const trigger = screen.getByRole("button", { name: label });
  await act(async () => {
    fireEvent.click(trigger);
  });
  await waitFor(() => {
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
  });
  return screen.findByRole("menu", { name: label });
}

describe("DocumentsScreen", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    // All independent mount reads have explicit native fixtures. An absent
    // Tauri bridge is not the error scenario exercised by document retry tests.
    vi.mocked(collectionIpc.getDocumentCollectionOrganization).mockResolvedValue({
      revision: 1, collections: [], memberships: [], ranges: [],
    });
    vi.mocked(artifactIpc.listLatestDocumentStudyGuideJobs).mockResolvedValue([]);
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue(null);
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([]);
    vi.mocked(documentIpc.getDocumentGenerationProgress).mockResolvedValue(null);
    vi.mocked(documentIpc.openDocumentSource).mockResolvedValue(undefined);
    vi.mocked(documentIpc.cancelDocumentFlashcardGeneration).mockResolvedValue(undefined);
    vi.mocked(studyGuideIpc.cancelDocumentStudyGuideGeneration).mockResolvedValue(undefined);
    vi.mocked(studyGuideIpc.generateDocumentStudyGuide).mockResolvedValue({
      revision: 2,
    } as studyGuideIpc.DocumentStudyGuide);
    vi.mocked(studyGuideIpc.getDocumentLearningPlan).mockResolvedValue(null);
    vi.mocked(studyGuideIpc.getDocumentStudyGuide).mockResolvedValue({ revision: 1 } as studyGuideIpc.DocumentStudyGuide);
  });

  it("shows a simple upload-first empty state without archive controls", async () => {
    renderDocuments();
    expect(await screen.findByRole("heading", { name: "No documents yet" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Upload document" })).toBeTruthy();
    expect(screen.queryByText(/show archived documents/iu)).toBeNull();
    expect(screen.queryByRole("button", { name: /archive|restore/iu })).toBeNull();
    expect(screen.queryByText(/review generated/u)).toBeNull();
    expect(screen.queryByText(/JSON/u)).toBeNull();
  });

  it("uploads one document, announces live progress, and keeps completed metadata behind info", async () => {
    const completed = makeDocument();
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({
      id: "1".repeat(32),
      purpose: "document",
      detectedKind: "pdf",
      displayName: "cardiac-physiology.pdf",
      mediaType: "application/pdf",
      byteSize: 128,
      sha256: "0".repeat(64),
      expiresAt: Date.now() + 60_000,
    });
    vi.mocked(documentIpc.importDocument).mockResolvedValue({
      documentId,
    });
    vi.mocked(documentIpc.startDocumentFlashcardGeneration).mockResolvedValue(undefined);
    vi.mocked(documentIpc.listDocuments)
      .mockResolvedValueOnce([])
      .mockResolvedValue([completed]);
    const { showToast, refreshDecks } = renderDocuments();
    await screen.findByRole("heading", { name: "No documents yet" });

    fireEvent.click(screen.getByRole("button", { name: "Upload document" }));

    await waitFor(() => {
      expect(platformFiles.selectExternalImport).toHaveBeenCalledWith("document");
      expect(showToast).toHaveBeenCalledWith(
        "info",
        "Document processing started. You can add more documents while this job runs.",
      );
      expect(documentIpc.importDocument).toHaveBeenCalledWith("1".repeat(32));
      expect(documentIpc.startDocumentFlashcardGeneration).toHaveBeenCalledWith(documentId, undefined);
      expect(refreshDecks).toHaveBeenCalled();
      expect(showToast).toHaveBeenCalledWith("success", "Created 7 flashcards in a new deck.");
    });
    expect(await screen.findByRole("heading", { name: "Cardiac physiology" })).toBeTruthy();
    expect(screen.getByText("cardiac-physiology.pdf")).toBeTruthy();
    expect(screen.getByText("7 cards")).toBeTruthy();
    expect(screen.queryByText("PDF · 2.0 KB")).toBeNull();


    await openDocumentActions();
    fireEvent.click(await screen.findByText("Generation info"));
    const dialog = await screen.findByRole("dialog", { name: "Cardiac physiology generation info" });
    expect(within(dialog).getByText("PDF · 2.0 KB")).toBeTruthy();
    expect(within(dialog).getByText("12 sec")).toBeTruthy();
    expect(within(dialog).getByText("45,678")).toBeTruthy();
  });

  it("shows detected non-PDF formats with truthful canonical section counts in metadata", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        originalFilename: "cardiac-physiology.docx",
        sourceFormat: "docx",
        sourceUnitKind: "section",
        sourceUnitCount: 8,
        pageCount: 8,
        extractedPageCount: 8,
        renderedPageCount: 0,
      }),
    ]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });

    await openDocumentActions();
    fireEvent.click(await screen.findByText("Generation info"));
    const dialog = await screen.findByRole("dialog", { name: "Cardiac physiology generation info" });
    expect(within(dialog).getByText("Word · 2.0 KB")).toBeTruthy();
    expect(within(dialog).getByText("Sections")).toBeTruthy();
    expect(within(dialog).getByText("8")).toBeTruthy();
    expect(screen.queryByText(/rendered/u)).toBeNull();
  });

  it("keeps completed OCR recovery out of permanent document-list copy", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        extractionMode: "ocr_fallback",
        ocrPageCount: 18,
        ocrCompletedPageCount: 18,
        ocrMeanConfidence: 0.91,
        ocrEngine: "Tesseract CLI via oxidize-pdf OcrProvider",
      }),
    ]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    expect(screen.queryByText(/Native PDF text was unreliable/u)).toBeNull();
    expect(screen.queryByText(/Mean OCR confidence/u)).toBeNull();
  });

  it("shows live OCR recovery progress after normal rendering completes", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        lifecycleState: "processing",
        generatedDeckId: null,
        generatedCardCount: 0,
        completedAt: null,
        extractionMode: "ocr_fallback",
        ocrPageCount: 12,
        ocrCompletedPageCount: 5,
        ocrMeanConfidence: null,
        ocrEngine: "tesseract-cli",
      }),
    ]);
    renderDocuments();

    expect((await screen.findAllByText("Recovering OCR 5/12")).length).toBeGreaterThan(0);
  });

  it("keeps upload controls available while another document is processing", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        lifecycleState: "processing",
        generatedDeckId: null,
        completedAt: null,
      }),
    ]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    expect((screen.getByRole("button", { name: "Upload document" }) as HTMLButtonElement).disabled)
      .toBe(false);
    expect((screen.getByRole("button", { name: "Upload images" }) as HTMLButtonElement).disabled)
      .toBe(false);
  });

  it("keeps a failed document in the library and offers a direct retry", async () => {
    const failed = makeDocument({
      lifecycleState: "failed",
      generatedDeckId: null,
      generatedNoteCount: 0,
      generatedCardCount: 0,
      safeErrorCode: "ai_initial_response_timeout",
      safeErrorMessage: "The provider never acknowledged the streamed request.",
    });
    const completed = makeDocument();
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([failed]);
    vi.mocked(documentIpc.startDocumentFlashcardGeneration).mockImplementation(async () => {
      vi.mocked(documentIpc.listDocuments).mockResolvedValue([completed]);
    });
    const { showToast } = renderDocuments();

    expect(await screen.findByText(failed.safeErrorMessage)).toBeTruthy();
    await waitFor(() => {
      expect((screen.getByRole("button", { name: "+ New collection" }) as HTMLButtonElement).disabled)
        .toBe(false);
      expect(artifactIpc.listLatestDocumentStudyGuideJobs).toHaveBeenCalled();
    });
    await openDocumentActions();
    fireEvent.click(await screen.findByText("Retry generation"));

    await waitFor(() => {
      expect(documentIpc.startDocumentFlashcardGeneration).toHaveBeenCalledWith(documentId, undefined);
      expect(showToast).toHaveBeenCalledWith("success", "Created 7 flashcards in a new deck.");
    });
  });

  it("shows a checked learning plan while final flashcard work remains", async () => {
    const processing = makeDocument({
      lifecycleState: "processing",
      generatedDeckId: null,
      generatedCardCount: 0,
      generationWindowCount: 8,
      generationCompletedWindowCount: 3,
      completedAt: null,
    });
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([processing]);
    vi.mocked(studyGuideIpc.getDocumentLearningPlan).mockResolvedValue({
      id: "e".repeat(32),
      documentId,
      revision: 1,
      compilerVersion: 1,
      outlineFingerprint: "f".repeat(64),
      planFingerprint: "1".repeat(64),
      summary: "Cardiac output depends on heart rate and stroke volume.",
      learningObjectives: ["Explain the determinants of cardiac output."],
      keyConcepts: ["cardiac output", "stroke volume"],
      topics: [{
        id: "2".repeat(32),
        parentTopicId: null,
        sortIndex: 0,
        title: "Cardiac output",
        summary: "Relates flow to rate and stroke volume.",
        learningObjectives: ["Relate heart rate and stroke volume to output."],
        keyConcepts: ["CO = HR × SV"],
        sourceLocator: {
          unitKind: "page",
          unitNumbers: [2, 3],
          sectionLabel: "Hemodynamics",
          assetIds: [],
        },
        evidenceChunkIndices: [1],
        evidenceHashes: ["3".repeat(64)],
        evidenceHash: "4".repeat(64),
        anchorHash: "5".repeat(64),
      }],
    });
    renderDocuments();

    const summary = await screen.findByText("Checked learning plan ready · 1 topic");
    fireEvent.click(summary);
    expect(await screen.findByText(
      "Cardiac output depends on heart rate and stroke volume.",
    )).toBeTruthy();
    expect(screen.getByText("Card authoring: 3 of 8 windows finished · 5 remaining.")).toBeTruthy();
    expect(screen.getByText(/nothing shown here is a provisional learner card/i)).toBeTruthy();
  });

  it("shows a lightweight local-processing phase before source-unit metadata is committed", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        lifecycleState: "processing",
        sourceUnitCount: 0,
        pageCount: 0,
        extractedPageCount: 0,
        renderedPageCount: 0,
        generatedDeckId: null,
        generatedCardCount: 0,
        completedAt: null,
      }),
    ]);
    renderDocuments();

    expect((await screen.findAllByText("Processing")).length).toBeGreaterThan(0);
    expect(screen.getAllByText("Extracting document content").length).toBeGreaterThan(0);
  });

  it("shows coarse local Xberg stages without inventing page progress", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        lifecycleState: "processing",
        sourceFormat: "docx",
        sourceUnitKind: "section",
        sourceUnitCount: 0,
        pageCount: 0,
        extractedPageCount: 0,
        renderedPageCount: 0,
        generatedDeckId: null,
        generatedCardCount: 0,
        completedAt: null,
      }),
    ]);
    vi.mocked(documentIpc.getDocumentGenerationProgress).mockResolvedValue(
      makeProgress("document_mapping"),
    );
    renderDocuments();

    expect((await screen.findAllByText("Mapping document structure")).length).toBeGreaterThan(0);
    expect(screen.getByText("Converting extracted headings and content into oStudy source blocks.")).toBeTruthy();
    expect(screen.queryByText(/Rendering pages/u)).toBeNull();
  });

  it("shows live rendered-page progress for PDFs once the total page count is known", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        lifecycleState: "processing",
        sourceUnitCount: 11,
        pageCount: 11,
        extractedPageCount: 11,
        renderedPageCount: 4,
        generatedDeckId: null,
        generatedCardCount: 0,
        completedAt: null,
      }),
    ]);
    renderDocuments();

    expect((await screen.findAllByText("Rendering pages 4/11")).length).toBeGreaterThan(0);
  });

  it("offers an exact stop action while a document generation task is processing", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        lifecycleState: "processing",
        generatedDeckId: null,
        completedAt: null,
      }),
    ]);
    const { showToast } = renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });

    await openDocumentActions();
    fireEvent.click(await screen.findByText("Stop generation"));

    await waitFor(() => {
      expect(documentIpc.cancelDocumentFlashcardGeneration).toHaveBeenCalledWith(documentId);
      expect(showToast).toHaveBeenCalledWith("info", "Stopping the current AI request…");
    });
    await waitFor(() => {
      expect(screen.queryByRole("menu", { name: "More actions for Cardiac physiology" })).toBeNull();
    });
    const stoppingMenu = await openDocumentActions();
    const stopping = within(stoppingMenu).getByRole("menuitem", { name: "Stopping…" });
    expect(stopping.getAttribute("aria-disabled")).toBe("true");
    expect(screen.queryByRole("button", { name: "Delete document" })).toBeNull();
  });

  it("generates an additional coverage pass into the existing deck", async () => {
    const existing = makeDocument();
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([existing]);
    vi.mocked(documentIpc.generateMoreDocumentFlashcards).mockResolvedValue({
      documentId,
      deckId: existing.generatedDeckId ?? "",
      createdNoteCount: 4,
      createdCardCount: 5,
      skippedDuplicateCount: 2,
    });
    const { showToast, refreshDecks } = renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    expect(screen.queryByRole("button", { name: "Generate more cards" })).toBeNull();

    await openDocumentActions();
    fireEvent.click(await screen.findByText("Generate more cards"));

    await waitFor(() => {
      expect(documentIpc.generateMoreDocumentFlashcards).toHaveBeenCalledWith(documentId, undefined);
      expect(refreshDecks).toHaveBeenCalled();
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Added 5 new flashcards to the existing deck.",
      );
    });
  });

  it("offers regeneration when the previously generated deck was deleted", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({ generatedDeckId: null, lifecycleState: "ready" }),
    ]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    expect(screen.queryByRole("button", { name: "Generate study guide" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Retry generation" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Generate more cards" })).toBeNull();

    await openDocumentActions();
    const menu = await screen.findByRole("menu", { name: "More actions for Cardiac physiology" });
    expect(within(menu).getByRole("menuitem", { name: "Retry generation" })).toBeTruthy();
    expect(within(menu).getByRole("menuitem", { name: "Generate study guide" })).toBeTruthy();
    expect(screen.getByText(/previous generated deck was deleted/u)).toBeTruthy();
    expect(screen.getByText(/retained document/u)).toBeTruthy();
  });

  it("keeps the ready tile limited to view, delete, and an overflow menu", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    expect(screen.getByRole("button", { name: "View" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Delete document" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "More actions for Cardiac physiology" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Generate study guide" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Generate more cards" })).toBeNull();

    await openDocumentActions();
    expect(await screen.findByText("Generation info")).toBeTruthy();
    expect(screen.getByText("Generate more cards")).toBeTruthy();
    expect(screen.getByText("Generate study guide")).toBeTruthy();
  });

  it("regenerates a prior study guide without regenerating flashcards", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    vi.mocked(artifactIpc.listLatestDocumentStudyGuideJobs).mockResolvedValue([{
      id: "c".repeat(32),
      documentId,
      planId: "d".repeat(32),
      artifactKind: "study_guide",
      state: "failed",
      totalUnits: 14,
      completedUnits: 0,
      safeErrorCode: "ai_document_output_invalid",
      safeErrorMessage: "The previous guide attempt failed validation.",
      apiTokenCount: null,
      apiTokenUsageComplete: false,
      createdAt: 1,
      startedAt: 2,
      updatedAt: 3,
      completedAt: 4,
    }]);
    const { showToast } = renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    expect(await screen.findByText("Needs attention")).toBeTruthy();
    await openDocumentActions();
    fireEvent.click(await screen.findByText("Regenerate study guide"));

    await waitFor(() => {
      expect(studyGuideIpc.generateDocumentStudyGuide).toHaveBeenCalledWith(documentId);
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Study guide revision 2 generated. Open it from Study Guides.",
      );
    });
    expect(documentIpc.startDocumentFlashcardGeneration).not.toHaveBeenCalled();
    expect(documentIpc.generateMoreDocumentFlashcards).not.toHaveBeenCalled();
  });

  it("exposes ready document content as a folder drag source without making row actions the drag handle", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    const source = document.querySelector(`[data-document-drag-source="${documentId}"]`) as HTMLElement;
    expect(source).toBeTruthy();
    expect(source.getAttribute("draggable")).toBe("true");

    const values = new Map<string, string>();
    const transfer = {
      effectAllowed: "none",
      dropEffect: "none",
      get types() { return Array.from(values.keys()); },
      setData(type: string, value: string) { values.set(type, value); },
      getData(type: string) { return values.get(type) ?? ""; },
    } as unknown as DataTransfer;
    fireEvent.dragStart(source, { dataTransfer: transfer });
    expect(transfer.getData(DOCUMENT_FOLDER_DRAG_TYPE)).toBe(documentId);
    expect(transfer.effectAllowed).toBe("copy");
  });

  it("opens the managed source through the compact View action", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    fireEvent.click(screen.getByRole("button", { name: "View" }));

    await waitFor(() => {
      expect(documentIpc.openDocumentSource).toHaveBeenCalledWith(documentId);
    });
  });

  it("permanently deletes a document only after precise confirmation and preserves citations", async () => {
    vi.mocked(documentIpc.listDocuments)
      .mockResolvedValueOnce([makeDocument()])
      .mockResolvedValue([]);
    vi.mocked(documentIpc.deleteDocument).mockResolvedValue({
      documentId,
      preservedDeckCitationCount: 1,
    });
    const { showToast, refreshDecks } = renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    fireEvent.click(screen.getByRole("button", { name: "Delete document" }));
    const dialog = await screen.findByRole("alertdialog", {
      name: "Delete this document permanently?",
    });
    expect(within(dialog).getByText(/structured source citations/iu)).toBeTruthy();
    expect(within(dialog).getByText(/source removed/iu)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete document" }));

    await waitFor(() => {
      expect(documentIpc.deleteDocument).toHaveBeenCalledWith(documentId, 2);
      expect(refreshDecks).toHaveBeenCalled();
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Document deleted. Preserved structured source citations for 1 deck.",
      );
    });
  });

  it("does not claim citation preservation when no surviving deck remains", async () => {
    const withoutDeck = makeDocument({
      generatedDeckId: null,
      lifecycleState: "ready",
      generatedCardCount: 7,
    });
    vi.mocked(documentIpc.listDocuments)
      .mockResolvedValueOnce([withoutDeck])
      .mockResolvedValue([]);
    vi.mocked(documentIpc.deleteDocument).mockResolvedValue({
      documentId,
      preservedDeckCitationCount: 0,
    });
    const { showToast } = renderDocuments();
    await screen.findByRole("heading", { name: "Cardiac physiology" });
    fireEvent.click(screen.getByRole("button", { name: "Delete document" }));
    const dialog = await screen.findByRole("alertdialog", {
      name: "Delete this document permanently?",
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete document" }));

    await waitFor(() => {
      expect(showToast).toHaveBeenCalledWith("success", "Document deleted from Documents.");
    });
  });
});

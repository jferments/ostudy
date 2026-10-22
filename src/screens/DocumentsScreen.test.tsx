import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as aiProfilesIpc from "../ipc/aiProfiles";
import * as artifactIpc from "../ipc/documentArtifactProgress";
import * as annotationIpc from "../ipc/documentAnnotations";
import * as figureIpc from "../ipc/documentFigures";
import * as documentIpc from "../ipc/documents";
import * as studyGuideIpc from "../ipc/studyGuides";
import * as studyMaterialsIpc from "../ipc/studyMaterials";
import * as subjectsIpc from "../ipc/subjects";
import * as platformFiles from "../platform/files";
import { makeSubject, makeSubjectLibrary } from "../test/factories";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { DocumentsScreen } from "./DocumentsScreen";

vi.mock("../platform/files", () => ({ selectExternalImport: vi.fn() }));
vi.mock("../ipc/aiProfiles", () => ({
  getAiTaskDefaults: vi.fn(),
  listAiProviderProfiles: vi.fn(),
}));
vi.mock("../ipc/subjects", () => ({
  getSubjectLibrary: vi.fn(),
  moveDocumentToSubject: vi.fn(),
  setDocumentStudyRanges: vi.fn(),
}));
vi.mock("../ipc/studyMaterials", () => ({
  getStudyTargetBundle: vi.fn(),
}));
vi.mock("../ipc/documentArtifactProgress", () => ({
  listLatestDocumentStudyGuideJobs: vi.fn(),
}));
vi.mock("../ipc/documentAnnotations", () => ({
  getDocumentAnnotations: vi.fn(),
  applyDocumentAnnotationChanges: vi.fn(),
  MAX_ANNOTATION_CHANGES_PER_REQUEST: 500,
  renderDocumentPageTile: vi.fn(),
}));
vi.mock("../ipc/documentFigures", () => ({
  getDocumentFigures: vi.fn(),
  findDocumentFigures: vi.fn(),
  setDocumentFiguresUseImages: vi.fn(),
  setDocumentFigureHidden: vi.fn(),
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
  renameDocument: vi.fn(),
  deleteDocument: vi.fn(),
}));
vi.mock("../ipc/studyGuides", () => ({
  generateDocumentStudyGuide: vi.fn(),
  getDocumentStudyGuide: vi.fn(),
  cancelDocumentStudyGuideGeneration: vi.fn(),
  getDocumentLearningPlan: vi.fn(),
  readDocumentSourceAsset: vi.fn(),
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

function renderDocuments(
  showToast = vi.fn(),
  refreshDecks = vi.fn().mockResolvedValue([]),
  props: { subjectId?: string | null; onOpenSubject?: (subjectId: string | null) => void; onSetUpAi?: () => void } = {},
) {
  render(
    <OverlayProvider>
      <DocumentsScreen refreshDecks={refreshDecks} showToast={showToast} {...props} />
    </OverlayProvider>,
  );
  return { showToast, refreshDecks };
}

async function openDocumentActions(title = "Cardiac physiology"): Promise<HTMLElement> {
  const label = `More actions for ${title}`;
  const trigger = screen.getByRole("button", { name: label });
  // Base UI connects a menu trigger to its root in a passive effect, so a click
  // that lands after the row renders but before React runs that effect does
  // nothing, and a busy machine widens the gap. An accepted click opens the menu
  // before act returns and a dropped one leaves it closed, so clicking until it
  // reports open never toggles it shut.
  for (let attempt = 0; attempt < 3 && trigger.getAttribute("aria-expanded") !== "true"; attempt += 1) {
    await act(async () => {
      fireEvent.click(trigger);
    });
  }
  await waitFor(() => {
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
  });
  return screen.findByRole("menu", { name: label });
}

async function chooseUpload(item: "Document…" | "Images…"): Promise<void> {
  fireEvent.click(screen.getByRole("button", { name: "Upload" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: item }));
}

async function chooseDocumentAction(name: string, title = "Cardiac physiology"): Promise<void> {
  const menu = await openDocumentActions(title);
  fireEvent.click(within(menu).getByRole("menuitem", { name }));
}

async function openGenerationDetails(title = "Cardiac physiology"): Promise<HTMLElement> {
  await chooseDocumentAction("Generation details", title);
  return screen.findByRole("dialog", { name: "Generation details" });
}

describe("DocumentsScreen", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    // All independent mount reads have explicit native fixtures. An absent
    // Tauri bridge is not the error scenario exercised by document retry tests.
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(makeSubjectLibrary());
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
    vi.mocked(aiProfilesIpc.getAiTaskDefaults).mockResolvedValue({ flashcardGenerationProfileId: "openrouter" });
    vi.mocked(aiProfilesIpc.listAiProviderProfiles).mockResolvedValue([
      { id: "openrouter" } as aiProfilesIpc.AiProviderProfileSummary,
    ]);
  });

  it("shows a simple upload-first empty state without archive controls", async () => {
    renderDocuments();
    expect(await screen.findByRole("heading", { name: "No documents yet" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Upload" })).toBeTruthy();
    expect(screen.queryByText(/show archived documents/iu)).toBeNull();
    expect(screen.queryByText(/Instructions for new document flashcards/iu)).toBeNull();
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
    // The library holds the document only once it is imported, which now
    // waits for the import dialog while Documents keeps polling.
    vi.mocked(documentIpc.listDocuments).mockImplementation(async () => (
      vi.mocked(documentIpc.importDocument).mock.calls.length > 0 ? [completed] : []
    ));
    const { showToast, refreshDecks } = renderDocuments();
    await screen.findByRole("heading", { name: "No documents yet" });

    await chooseUpload("Document…");
    // A PDF asks whether to use its images, which are on unless turned off.
    const importDialog = await screen.findByRole("dialog", { name: "Import document" });
    expect(within(importDialog).getByText("cardiac-physiology.pdf")).toBeTruthy();
    expect(within(importDialog).getByRole("checkbox", { name: "Use images from this document" })
      .getAttribute("aria-checked")).toBe("true");
    expect(documentIpc.importDocument).not.toHaveBeenCalled();
    fireEvent.click(within(importDialog).getByRole("button", { name: "Import" }));

    await waitFor(() => {
      expect(platformFiles.selectExternalImport).toHaveBeenCalledWith("document");
      expect(showToast).toHaveBeenCalledWith(
        "info",
        "Document processing started. You can add more documents while this job runs.",
      );
      expect(documentIpc.importDocument).toHaveBeenCalledWith("1".repeat(32), { useImages: true });
      expect(documentIpc.startDocumentFlashcardGeneration).toHaveBeenCalledWith(documentId, undefined);
      expect(refreshDecks).toHaveBeenCalled();
      expect(showToast).toHaveBeenCalledWith("success", "Created 7 flashcards in a new deck.");
    });
    expect(await screen.findByRole("heading", { name: "Cardiac physiology" })).toBeTruthy();
    expect(screen.getByText("7 cards")).toBeTruthy();
    expect(screen.getByText(/^PDF · 10 pages · added /u)).toBeTruthy();
    expect(screen.queryByText("PDF · 2.0 KB")).toBeNull();

    await chooseDocumentAction("Details");
    const dialog = await screen.findByRole("dialog", { name: "Details" });
    expect(within(dialog).getByText("cardiac-physiology.pdf")).toBeTruthy();
    expect(within(dialog).getByText("PDF · 2.0 KB")).toBeTruthy();
    expect(within(dialog).getByText("12 sec")).toBeTruthy();
    expect(within(dialog).getByText("45,678")).toBeTruthy();
  });

  it("imports a PDF without its images when the learner turns them off, and not at all on cancel", async () => {
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({
      id: "1".repeat(32),
      purpose: "document",
      detectedKind: "pdf",
      displayName: "renal.pdf",
      mediaType: "application/pdf",
      byteSize: 128,
      sha256: "0".repeat(64),
      expiresAt: Date.now() + 60_000,
    });
    vi.mocked(documentIpc.importDocument).mockResolvedValue({ documentId });
    vi.mocked(documentIpc.startDocumentFlashcardGeneration).mockResolvedValue(undefined);
    renderDocuments();
    await screen.findByRole("heading", { name: "No documents yet" });

    await chooseUpload("Document…");
    let importDialog = await screen.findByRole("dialog", { name: "Import document" });
    fireEvent.click(within(importDialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Import document" })).toBeNull());
    expect(documentIpc.importDocument).not.toHaveBeenCalled();

    await chooseUpload("Document…");
    importDialog = await screen.findByRole("dialog", { name: "Import document" });
    fireEvent.click(within(importDialog).getByRole("checkbox", { name: "Use images from this document" }));
    fireEvent.click(within(importDialog).getByRole("button", { name: "Import" }));
    await waitFor(() => {
      expect(documentIpc.importDocument).toHaveBeenCalledWith("1".repeat(32), { useImages: false });
      expect(documentIpc.startDocumentFlashcardGeneration).toHaveBeenCalledWith(documentId, undefined);
    });
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

    await chooseDocumentAction("Details");
    const dialog = await screen.findByRole("dialog", { name: "Details" });
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
    expect((screen.getByRole("button", { name: "Upload" }) as HTMLButtonElement).disabled)
      .toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Upload" }));
    expect((await screen.findByRole("menuitem", { name: "Images…" })).getAttribute("aria-disabled")).not.toBe("true");
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
      expect(subjectsIpc.getSubjectLibrary).toHaveBeenCalled();
      expect(artifactIpc.listLatestDocumentStudyGuideJobs).toHaveBeenCalled();
    });
    await chooseDocumentAction("Retry");

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

    const row = await screen.findByTestId("document-row-progress");
    expect(within(row).getByText("Extracting document content")).toBeTruthy();
    // The bar replaces the row's status badge and its Open button.
    expect(screen.queryByText("Processing")).toBeNull();
    expect(screen.queryByRole("button", { name: "Open" })).toBeNull();
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

    await waitFor(() => expect(within(screen.getByTestId("document-row-progress")).getByText("Mapping document structure")).toBeTruthy());
    expect(screen.queryByText("Converting extracted headings and content into oStudy source blocks.")).toBeNull();
    const details = await openGenerationDetails();
    expect(within(details).getByText("Converting extracted headings and content into oStudy source blocks.")).toBeTruthy();
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

    await chooseDocumentAction("Stop");

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
    expect(within(stoppingMenu).queryByRole("menuitem", { name: "Delete" })).toBeNull();
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
    expect(screen.queryByRole("button", { name: "Make more cards…" })).toBeNull();

    await chooseDocumentAction("Make more cards…");
    const dialog = await screen.findByRole("dialog", { name: "Make more cards" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Make cards" }));

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
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    expect(screen.getByText("7 cards · deck deleted")).toBeTruthy();

    const menu = await openDocumentActions();
    expect(within(menu).getByRole("menuitem", { name: "Retry" })).toBeTruthy();
    expect(within(menu).getByRole("menuitem", { name: "Make a study guide" })).toBeTruthy();
    expect(within(menu).queryByRole("menuitem", { name: "Make more cards…" })).toBeNull();
  });

  it("keeps a ready row to Open and one menu, with no model or instructions on the page", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    const row = screen.getByTestId("document-row");
    expect(within(row).getAllByRole("button").map((button) => button.getAttribute("aria-label") ?? button.textContent))
      .toEqual(["Open", "More actions for Cardiac physiology"]);
    expect(screen.queryByText(/instructions/iu)).toBeNull();
    expect(screen.queryByText(/model/iu)).toBeNull();

    const menu = await openDocumentActions();
    expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "Annotate", "Figures", "Make more cards…", "Make a study guide", "Rename…", "Details", "Delete",
    ]);
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
    await chooseDocumentAction("Remake the study guide");

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

  it("shows a fresh guide bar as soon as a finished guide is remade", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    vi.mocked(artifactIpc.listLatestDocumentStudyGuideJobs).mockResolvedValue([{
      id: "c".repeat(32), documentId, planId: "d".repeat(32), artifactKind: "study_guide",
      state: "succeeded", totalUnits: 14, completedUnits: 14, safeErrorCode: "", safeErrorMessage: "",
      apiTokenCount: 1_000, apiTokenUsageComplete: true,
      createdAt: 1, startedAt: 2, updatedAt: 3, completedAt: 4,
    }]);
    // The new run is still rebuilding the outline: it has no record of its own yet.
    vi.mocked(studyGuideIpc.generateDocumentStudyGuide).mockReturnValue(new Promise(() => undefined));
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    await chooseDocumentAction("Remake the study guide");

    // The finished guide's full bar does not linger; the row shows the new run starting.
    const row = await screen.findByTestId("document-row-progress");
    expect(within(row).getByText("Making a study guide")).toBeTruthy();
    expect(within(row).getByRole("progressbar").getAttribute("aria-valuenow")).toBeNull();
    const details = await openGenerationDetails();
    expect(within(details).getByText("Preparing")).toBeTruthy();
    expect(within(details).queryByText(/Study Guide usage/u)).toBeNull();
  });

  it("opens AI setup from a document's Make actions while no model is set", async () => {
    vi.mocked(aiProfilesIpc.getAiTaskDefaults).mockResolvedValue({ flashcardGenerationProfileId: null });
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    const onSetUpAi = vi.fn();
    renderDocuments(vi.fn(), vi.fn().mockResolvedValue([]), { onSetUpAi });

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    await waitFor(() => expect(aiProfilesIpc.listAiProviderProfiles).toHaveBeenCalled());
    await chooseDocumentAction("Make a study guide");
    expect(onSetUpAi).toHaveBeenCalledTimes(1);
    await chooseDocumentAction("Make more cards…");
    expect(onSetUpAi).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("dialog", { name: "Make more cards" })).toBeNull();
    expect(studyGuideIpc.generateDocumentStudyGuide).not.toHaveBeenCalled();
  });

  it("makes the study guide itself once a model is set", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    const onSetUpAi = vi.fn();
    renderDocuments(vi.fn(), vi.fn().mockResolvedValue([]), { onSetUpAi });

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    await waitFor(() => expect(aiProfilesIpc.listAiProviderProfiles).toHaveBeenCalled());
    await chooseDocumentAction("Make a study guide");
    await waitFor(() => expect(studyGuideIpc.generateDocumentStudyGuide).toHaveBeenCalledWith(documentId));
    expect(onSetUpAi).not.toHaveBeenCalled();
  });

  it("opens a PDF's pages for annotation and returns to the list", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    vi.mocked(annotationIpc.getDocumentAnnotations).mockResolvedValue({ documentId, pages: [], annotations: [] });
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    await chooseDocumentAction("Annotate");

    expect(await screen.findByText("No pages to annotate")).toBeTruthy();
    expect(annotationIpc.getDocumentAnnotations).toHaveBeenCalledWith(documentId);
    fireEvent.click(screen.getByRole("button", { name: "Back to documents" }));
    expect(await screen.findByRole("heading", { name: "Cardiac physiology" })).toBeTruthy();
    expect(annotationIpc.applyDocumentAnnotationChanges).not.toHaveBeenCalled();
  });

  it("opens a PDF's figures and returns to the list", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    vi.mocked(figureIpc.getDocumentFigures).mockResolvedValue({
      documentId, useImages: true, state: "none", scannedAt: null, reading: "none", readAt: null, figures: [],
    });
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    await chooseDocumentAction("Figures");

    expect(await screen.findByText("No figures found yet")).toBeTruthy();
    expect(figureIpc.getDocumentFigures).toHaveBeenCalledWith(documentId);
    fireEvent.click(screen.getByRole("button", { name: "Back to documents" }));
    expect(await screen.findByRole("heading", { name: "Cardiac physiology" })).toBeTruthy();
  });

  it("offers annotation only for PDFs whose pages were drawn", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({ originalFilename: "cardiac-physiology.docx", sourceFormat: "docx", renderedPageCount: 0 }),
      makeDocument({ id: "c".repeat(32), title: "Renal physiology", lifecycleState: "processing", renderedPageCount: 4 }),
    ]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    await screen.findByRole("heading", { name: "Renal physiology" });
    expect(screen.queryByRole("button", { name: "Annotate" })).toBeNull();
  });

  it("opens the managed source from the row's Open button", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    fireEvent.click(screen.getByRole("button", { name: "Open" }));

    await waitFor(() => {
      expect(documentIpc.openDocumentSource).toHaveBeenCalledWith(documentId);
    });
  });

  it("renames a document from its menu", async () => {
    vi.mocked(documentIpc.listDocuments)
      .mockResolvedValueOnce([makeDocument()])
      .mockResolvedValue([makeDocument({ title: "Cardiac cycle", revision: 3 })]);
    vi.mocked(documentIpc.renameDocument).mockResolvedValue({ documentId, title: "Cardiac cycle", revision: 3 });
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    await chooseDocumentAction("Rename…");
    const dialog = await screen.findByRole("dialog", { name: "Rename document" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Title" }), { target: { value: "  Cardiac cycle " } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(documentIpc.renameDocument).toHaveBeenCalledWith(documentId, 2, "Cardiac cycle");
      expect(screen.queryByRole("dialog", { name: "Rename document" })).toBeNull();
    });
    expect(await screen.findByRole("heading", { name: "Cardiac cycle" })).toBeTruthy();
  });

  it("keeps the rename dialog open with the error when saving fails", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    vi.mocked(documentIpc.renameDocument).mockRejectedValue(new Error("This document changed. Reload it and try again."));
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    await chooseDocumentAction("Rename…");
    const dialog = await screen.findByRole("dialog", { name: "Rename document" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Title" }), { target: { value: "New title" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));

    expect(await within(dialog).findByText("This document changed. Reload it and try again.")).toBeTruthy();
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
    await chooseDocumentAction("Delete");
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
    await chooseDocumentAction("Delete");
    const dialog = await screen.findByRole("alertdialog", {
      name: "Delete this document permanently?",
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete document" }));

    await waitFor(() => {
      expect(showToast).toHaveBeenCalledWith("success", "Document deleted from Documents.");
    });
  });

  describe("with subjects", () => {
    const cardiology = makeSubject({ id: "e".repeat(32), name: "Cardiology", sortOrder: 0 });
    const renal = makeSubject({ id: "f".repeat(32), name: "Renal", sortOrder: 1 });
    const looseId = "b".repeat(32);
    const library = makeSubjectLibrary({
      documentFilingRevision: 4,
      subjects: [cardiology, renal],
      documents: [{ documentId, subjectId: cardiology.id }, { documentId: looseId, subjectId: null }],
      studyRanges: [{
        id: "1".repeat(32), documentId, unitKind: "page", startUnit: 1, endUnit: 3, sourceSha256: "a".repeat(64), createdAt: 1,
      }],
    });
    const loose = () => makeDocument({ id: looseId, title: "Medical terminology" });

    beforeEach(() => {
      vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(library);
      vi.mocked(studyMaterialsIpc.getStudyTargetBundle).mockResolvedValue(null);
      vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument({ sha256: "a".repeat(64) }), loose()]);
    });

    it("groups every document under its subject, with No subject last and Add to… on its rows", async () => {
      const onOpenSubject = vi.fn();
      renderDocuments(vi.fn(), vi.fn().mockResolvedValue([]), { onOpenSubject });

      const groups = await screen.findAllByTestId("document-group");
      expect(groups.map((group) => within(group).getByRole("heading", { level: 2 }).textContent))
        .toEqual(["Cardiology1 document", "No subject1 document"]);
      expect(within(groups[0]!).getByText(/pages 1–3 studied/u)).toBeTruthy();
      expect(within(groups[1]!).getByRole("button", { name: "Add to…" })).toBeTruthy();
      expect(screen.queryByText(/collection/iu)).toBeNull();
      fireEvent.click(within(groups[0]!).getByRole("button", { name: "Open Cardiology" }));
      expect(onOpenSubject).toHaveBeenCalledWith(cardiology.id);
    });

    it("narrows the groups to the documents a search matches", async () => {
      renderDocuments(vi.fn(), vi.fn().mockResolvedValue([]));

      await screen.findAllByTestId("document-group");
      const search = screen.getByRole("searchbox", { name: "Search documents" });
      fireEvent.change(search, { target: { value: "TERMINOLOGY" } });
      const groups = screen.getAllByTestId("document-group");
      expect(groups.map((group) => within(group).getByRole("heading", { level: 2 }).textContent))
        .toEqual(["No subject1 document"]);

      fireEvent.change(search, { target: { value: "pharmacology" } });
      expect(screen.queryAllByTestId("document-group")).toEqual([]);
      expect(screen.getByText("No documents match “pharmacology”")).toBeTruthy();
    });

    it("lists one subject's documents with its study materials action and no collection", async () => {
      renderDocuments(vi.fn(), vi.fn().mockResolvedValue([]), { subjectId: cardiology.id });

      await screen.findByRole("heading", { name: "Cardiac physiology" });
      expect(screen.queryByRole("heading", { name: "Medical terminology" })).toBeNull();
      expect(await screen.findByRole("button", { name: /Make study materials/u })).toBeTruthy();
      expect(screen.queryByText(/collection/iu)).toBeNull();
      const menu = await openDocumentActions();
      expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent)).toEqual(expect.arrayContaining([
        "Reading range…", "Move to another subject…", "Remove from subject",
      ]));
    });

    it("opens AI setup from Make study materials while the chosen service is gone", async () => {
      vi.mocked(aiProfilesIpc.listAiProviderProfiles).mockResolvedValue([]);
      const onSetUpAi = vi.fn();
      renderDocuments(vi.fn(), vi.fn().mockResolvedValue([]), { subjectId: cardiology.id, onSetUpAi });

      const make = await screen.findByRole("button", { name: /Make study materials/u });
      await waitFor(() => expect(aiProfilesIpc.listAiProviderProfiles).toHaveBeenCalled());
      await act(async () => undefined);
      fireEvent.click(make);
      expect(onSetUpAi).toHaveBeenCalledTimes(1);
      expect(screen.queryByRole("dialog", { name: "Make study materials" })).toBeNull();
    });

    it("files a loose document in a subject from Add to…", async () => {
      vi.mocked(subjectsIpc.moveDocumentToSubject).mockResolvedValue({
        ...library,
        documentFilingRevision: 5,
        documents: [{ documentId, subjectId: cardiology.id }, { documentId: looseId, subjectId: renal.id }],
      });
      renderDocuments(vi.fn(), vi.fn().mockResolvedValue([]), { subjectId: null });

      fireEvent.click(await screen.findByRole("button", { name: "Add to…" }));
      const sheet = await screen.findByRole("dialog", { name: "Move “Medical terminology”" });
      fireEvent.click(within(sheet).getByRole("radio", { name: "Renal" }));
      fireEvent.click(within(sheet).getByRole("button", { name: "Move" }));

      await waitFor(() => expect(subjectsIpc.moveDocumentToSubject).toHaveBeenCalledWith({
        documentId: looseId, subjectId: renal.id, expectedDocumentFilingRevision: 4,
      }));
      expect(await screen.findByRole("heading", { name: "Every document has a subject" })).toBeTruthy();
    });

    it("sets a document's reading range in its subject", async () => {
      vi.mocked(subjectsIpc.setDocumentStudyRanges).mockResolvedValue({ ...library, documentFilingRevision: 5, studyRanges: [] });
      renderDocuments(vi.fn(), vi.fn().mockResolvedValue([]), { subjectId: cardiology.id });

      await screen.findByRole("heading", { name: "Cardiac physiology" });
      await chooseDocumentAction("Reading range…");
      const dialog = await screen.findByRole("dialog", { name: "Reading range" });
      const field = within(dialog).getByRole("textbox", { name: "Pages" }) as HTMLInputElement;
      expect(field.value).toBe("1-3");
      fireEvent.change(field, { target: { value: "2-4, 7" } });
      fireEvent.click(within(dialog).getByRole("button", { name: "Save range" }));

      await waitFor(() => expect(subjectsIpc.setDocumentStudyRanges).toHaveBeenCalledWith({
        documentId,
        ranges: [{ unitKind: "page", startUnit: 2, endUnit: 4 }, { unitKind: "page", startUnit: 7, endUnit: 7 }],
        expectedDocumentFilingRevision: 4,
      }));
    });

    it("uploads into the chosen subject before processing starts", async () => {
      vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({ id: "1".repeat(32) } as
        NonNullable<Awaited<ReturnType<typeof platformFiles.selectExternalImport>>>);
      vi.mocked(documentIpc.importDocument).mockResolvedValue({ documentId: looseId });
      vi.mocked(documentIpc.startDocumentFlashcardGeneration).mockResolvedValue(undefined);
      vi.mocked(subjectsIpc.moveDocumentToSubject).mockResolvedValue(library);
      renderDocuments(vi.fn(), vi.fn().mockResolvedValue([]), { subjectId: renal.id });

      await screen.findByRole("heading", { name: "No documents in this subject yet" });
      await chooseUpload("Document…");

      await waitFor(() => {
        expect(subjectsIpc.moveDocumentToSubject).toHaveBeenCalledWith(expect.objectContaining({ documentId: looseId, subjectId: renal.id }));
        expect(documentIpc.startDocumentFlashcardGeneration).toHaveBeenCalledWith(looseId, undefined);
      });
      expect(vi.mocked(subjectsIpc.moveDocumentToSubject).mock.invocationCallOrder[0])
        .toBeLessThan(vi.mocked(documentIpc.startDocumentFlashcardGeneration).mock.invocationCallOrder[0] ?? 0);
    });
  });
});

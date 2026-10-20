import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as documentArtifactProgress from "../../ipc/documentArtifactProgress";
import * as documentIpc from "../../ipc/documents";
import * as studyGuideIpc from "../../ipc/studyGuides";
import * as subjects from "../../ipc/subjects";
import { makeSubjectLibrary } from "../../test/factories";
import { LayoutModeProvider } from "../../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { DocumentsScreen } from "../DocumentsScreen";
import { matchesLayoutQuery, layoutWidths } from "../../test/viewport";

vi.mock("../../platform/files", () => ({ selectExternalImport: vi.fn() }));
vi.mock("../../ipc/documentArtifactProgress", () => ({
  listLatestDocumentStudyGuideJobs: vi.fn(),
}));
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
vi.mock("../../ipc/subjects", () => ({
  getSubjectLibrary: vi.fn(),
  moveDocumentToSubject: vi.fn(),
  setDocumentStudyRanges: vi.fn(),
}));

const documentId = "a".repeat(32);
const originalMatchMedia = window.matchMedia;

function setCompactLayout(compact: boolean): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: matchesLayoutQuery(query, layoutWidths[compact ? "compact" : "wide"]),
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

afterEach(() => {
  window.matchMedia = originalMatchMedia;
});

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

function renderDocuments(compact = false) {
  setCompactLayout(compact);
  const showToast = vi.fn();
  render(
    <LayoutModeProvider>
      <OverlayProvider>
        <DocumentsScreen showToast={showToast} refreshDecks={async () => []} />
      </OverlayProvider>
    </LayoutModeProvider>,
  );
  return showToast;
}

async function openDocumentActions(
  title = "Cardiac physiology",
  presentation: "menu" | "dialog" = "menu",
): Promise<HTMLElement> {
  const label = `More actions for ${title}`;
  const trigger = screen.getByRole("button", { name: label });
  // Base UI drops a click that lands before it wires the trigger, which then reports closed.
  await waitFor(() => expect(trigger.getAttribute("aria-expanded")).toBe("false"));
  fireEvent.click(trigger);
  return screen.findByRole(presentation, { name: label });
}

async function waitForFilingDiscovery(): Promise<void> {
  await waitFor(() => expect(subjects.getSubjectLibrary).toHaveBeenCalled());
  await act(async () => {
    await Promise.resolve();
  });
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(subjects.getSubjectLibrary).mockResolvedValue(makeSubjectLibrary());
  vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
  vi.mocked(documentArtifactProgress.listLatestDocumentStudyGuideJobs).mockResolvedValue([]);
  vi.mocked(documentIpc.getDocumentGenerationProgress).mockResolvedValue(null);
  vi.mocked(documentIpc.startDocumentFlashcardGeneration).mockResolvedValue(undefined);
  vi.mocked(studyGuideIpc.generateDocumentStudyGuide).mockResolvedValue({ revision: 1 } as studyGuideIpc.DocumentStudyGuide);
  vi.mocked(studyGuideIpc.cancelDocumentStudyGuideGeneration).mockResolvedValue(undefined);
});

describe("Document job presentation", () => {
  it("keeps document actions open while subject filing discovery settles", async () => {
    let finish!: (value: subjects.SubjectLibrary) => void;
    vi.mocked(subjects.getSubjectLibrary).mockReturnValueOnce(
      new Promise((resolve) => { finish = resolve; }),
    );
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({ generatedDeckId: null, generatedCardCount: 0 }),
    ]);
    // This test owns the filing-discovery race. Keep the independent
    // progress reads pending so their rerenders cannot close the menu and turn
    // this into a scheduler-timing test.
    vi.mocked(documentArtifactProgress.listLatestDocumentStudyGuideJobs).mockReturnValueOnce(
      new Promise(() => undefined),
    );
    vi.mocked(documentIpc.getDocumentGenerationProgress).mockReturnValueOnce(
      new Promise(() => undefined),
    );
    renderDocuments(true);

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    const actions = await openDocumentActions("Cardiac physiology", "dialog");
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();

    await act(async () => {
      finish(makeSubjectLibrary());
      await Promise.resolve();
    });
    expect(actions.isConnected).toBe(true);
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
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
    await waitForFilingDiscovery();
    await openDocumentActions();
    fireEvent.click(await screen.findByText("Retry"));

    await waitFor(() => expect(toast).toHaveBeenCalledWith(
      "success",
      "Created 7 flashcards in a new deck.",
    ));
    expect(screen.queryByText(/Analysis is partial|knowledge proposals|source dispositions/iu)).toBeNull();
    expect(JSON.stringify(toast.mock.calls)).not.toMatch(/partial|unresolved/iu);
  });

  it("offers stop for a durable guide that is still running after flashcards finish", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    vi.mocked(documentArtifactProgress.listLatestDocumentStudyGuideJobs).mockResolvedValue([{
      id: "c".repeat(32),
      documentId,
      planId: "d".repeat(32),
      artifactKind: "study_guide",
      state: "generating",
      totalUnits: 10,
      completedUnits: 4,
      safeErrorCode: "",
      safeErrorMessage: "",
      apiTokenCount: null,
      apiTokenUsageComplete: false,
      createdAt: 1,
      startedAt: 2,
      updatedAt: 3,
      completedAt: null,
    }]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    await waitFor(() => expect(screen.getByText("4 / 10 sections")).toBeTruthy());
    expect(studyGuideIpc.generateDocumentStudyGuide).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: /study guide/iu })).toBeNull();

    await openDocumentActions();
    fireEvent.click(await screen.findByText("Stop study guide"));
    await waitFor(() => expect(
      studyGuideIpc.cancelDocumentStudyGuideGeneration,
    ).toHaveBeenCalledWith(documentId));
  });

  it("keeps durable guide failures visible without hiding saved flashcards", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    vi.mocked(documentArtifactProgress.listLatestDocumentStudyGuideJobs).mockResolvedValue([{
      id: "c".repeat(32),
      documentId,
      planId: "d".repeat(32),
      artifactKind: "study_guide",
      state: "failed",
      totalUnits: 10,
      completedUnits: 4,
      safeErrorCode: "ai_provider_failed",
      safeErrorMessage: "The provider could not finish the guide.",
      apiTokenCount: null,
      apiTokenUsageComplete: false,
      createdAt: 1,
      startedAt: 2,
      updatedAt: 3,
      completedAt: 4,
    }]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    expect(await screen.findByText("Needs attention")).toBeTruthy();
    expect(screen.getByText("The provider could not finish the guide.")).toBeTruthy();
    expect(screen.getByText("7 cards")).toBeTruthy();
    expect(studyGuideIpc.generateDocumentStudyGuide).not.toHaveBeenCalled();
  });
});

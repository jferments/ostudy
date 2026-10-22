import { useState } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DocumentSummary } from "../ipc/documents";
import { DocumentJobsProvider, useDocumentJobs } from "./DocumentJobsProvider";

const mocks = vi.hoisted(() => ({
  list: vi.fn(), guide: vi.fn(), cancelGuide: vi.fn(), cancelDocument: vi.fn(),
  progress: vi.fn(), plan: vi.fn(), start: vi.fn(), legacyCards: vi.fn(), toast: vi.fn(),
  refreshDecks: vi.fn(), confirm: vi.fn(),
}));
vi.mock("../ipc/documents", () => ({
  listDocuments: mocks.list, getDocumentGenerationProgress: mocks.progress,
  cancelDocumentFlashcardGeneration: mocks.cancelDocument, deleteDocument: vi.fn(),
  generateMoreDocumentFlashcards: vi.fn(), importDocument: vi.fn(),
  openDocumentSource: vi.fn(), startDocumentFlashcardGeneration: mocks.start,
  retryDocumentFlashcardGeneration: mocks.legacyCards,
}));
vi.mock("../ipc/studyGuides", () => ({
  generateDocumentStudyGuide: mocks.guide,
  cancelDocumentStudyGuideGeneration: mocks.cancelGuide,
  getDocumentLearningPlan: mocks.plan,
}));
vi.mock("../platform/files", () => ({ selectExternalImport: vi.fn() }));
vi.mock("../ui/overlays/overlayContext", () => ({ useConfirm: () => mocks.confirm }));

const document = { id: "a".repeat(32), title: "Geometry", revision: 1,
  lifecycleState: "ready", generatedDeckId: "b".repeat(32), analysisPendingItems: 0,
} as unknown as DocumentSummary;
const retryDocument = {
  ...document,
  id: "c".repeat(32),
  lifecycleState: "failed",
  generatedDeckId: null,
  generatedCardCount: 0,
  safeErrorCode: "",
  safeErrorMessage: "",
} as DocumentSummary;
function DocumentsRoute() {
  const jobs = useDocumentJobs();
  if (!jobs) throw new Error("Missing job owner");
  return <section aria-label="Documents route">
    <button onClick={() => { void jobs.stopStudyGuide(document); }}>Stop guide</button>
    {jobs.errorMessage ? <p role="alert">{jobs.errorMessage}</p> : null}
    <button onClick={() => { void jobs.retry(retryDocument); }}>Retry cards</button>
    <button onClick={() => { void jobs.refreshDocuments(); }}>Refresh documents</button>
  </section>;
}
function Routes() {
  const [documents, setDocuments] = useState(true);
  return <>
    <button onClick={() => setDocuments((previous) => !previous)}>Navigate</button>
    {documents ? <DocumentsRoute /> : <p>Another route</p>}
  </>;
}
function open() {
  render(<DocumentJobsProvider refreshDecks={mocks.refreshDecks} showToast={mocks.toast}>
    <Routes />
  </DocumentJobsProvider>);
}
beforeEach(() => {
  vi.clearAllMocks();
  mocks.list.mockResolvedValue([document]);
  mocks.progress.mockResolvedValue(null);
  mocks.plan.mockResolvedValue(null);
  mocks.cancelGuide.mockResolvedValue(undefined);
  mocks.cancelDocument.mockResolvedValue(undefined);
  mocks.start.mockResolvedValue(undefined);
  mocks.legacyCards.mockResolvedValue(undefined);
  mocks.refreshDecks.mockResolvedValue(undefined);
  mocks.confirm.mockResolvedValue(true);
});

describe("application-lifetime document jobs", () => {
  it("does not launch a duplicate frontend guide when native cards finish, including after navigation", async () => {
    let current = [retryDocument];
    mocks.list.mockImplementation(async () => current);
    mocks.start.mockImplementation(async () => {
      current = [{ ...retryDocument, lifecycleState: "processing" }];
    });
    open();
    await waitFor(() => expect(mocks.list).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Retry cards" }));
    await waitFor(() => expect(mocks.start).toHaveBeenCalledWith(retryDocument.id, undefined, undefined));

    fireEvent.click(screen.getByRole("button", { name: "Navigate" }));
    expect(mocks.cancelGuide).not.toHaveBeenCalled();
    expect(mocks.cancelDocument).not.toHaveBeenCalled();
    current = [{ ...retryDocument, lifecycleState: "ready", generatedDeckId: "d".repeat(32), generatedCardCount: 6 }];
    fireEvent.click(screen.getByRole("button", { name: "Navigate" }));
    fireEvent.click(screen.getByRole("button", { name: "Refresh documents" }));
    await waitFor(() => expect(mocks.toast).toHaveBeenCalledWith("success", "Created 6 flashcards in a new deck."));
    expect(mocks.guide).not.toHaveBeenCalled();
    expect(mocks.start).toHaveBeenCalledTimes(1);
    expect(mocks.refreshDecks).toHaveBeenCalled();
  });

  it("hands flashcard generation to native ownership and observes durable completion", async () => {
    let current = [retryDocument];
    mocks.list.mockImplementation(async () => current);
    mocks.start.mockImplementation(async () => {
      current = [{ ...retryDocument, lifecycleState: "processing" }];
    });
    open();
    await waitFor(() => expect(mocks.list).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Retry cards" }));
    await waitFor(() => expect(mocks.start).toHaveBeenCalledWith(retryDocument.id, undefined, undefined));
    expect(mocks.legacyCards).not.toHaveBeenCalled();
    current = [{
      ...retryDocument, lifecycleState: "ready", generatedDeckId: "d".repeat(32),
      generatedCardCount: 6, safeErrorCode: "", safeErrorMessage: "",
    }];
    fireEvent.click(screen.getByRole("button", { name: "Refresh documents" }));
    await waitFor(() => expect(mocks.toast).toHaveBeenCalledWith("success", "Created 6 flashcards in a new deck."));
    expect(mocks.refreshDecks).toHaveBeenCalled();
  });

  it("explicit stop reaches a native guide even when no frontend guide promise owns it", async () => {
    open();
    await waitFor(() => expect(mocks.list).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Stop guide" }));
    await waitFor(() => expect(mocks.cancelGuide).toHaveBeenCalledWith(document.id));
    expect(mocks.guide).not.toHaveBeenCalled();
  });
});

import { useState } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DocumentSummary } from "../ipc/documents";
import { DocumentJobsProvider, useDocumentJobs } from "./DocumentJobsProvider";

const mocks = vi.hoisted(() => ({
  list: vi.fn(), guide: vi.fn(), cancelGuide: vi.fn(), cancelDocument: vi.fn(),
  progress: vi.fn(), toast: vi.fn(), refreshDecks: vi.fn(), confirm: vi.fn(),
}));
vi.mock("../ipc/documents", () => ({
  listDocuments: mocks.list, getDocumentGenerationProgress: mocks.progress,
  cancelDocumentFlashcardGeneration: mocks.cancelDocument, deleteDocument: vi.fn(),
  generateMoreDocumentFlashcards: vi.fn(), importDocument: vi.fn(),
  openDocumentSource: vi.fn(), retryDocumentFlashcardGeneration: vi.fn(),
}));
vi.mock("../ipc/studyGuides", () => ({
  generateDocumentStudyGuide: mocks.guide, cancelDocumentStudyGuideGeneration: mocks.cancelGuide,
}));
vi.mock("../platform/files", () => ({ selectExternalImport: vi.fn() }));
vi.mock("../ui/overlays/overlayContext", () => ({ useConfirm: () => mocks.confirm }));

const document = { id: "a".repeat(32), title: "Geometry", revision: 1,
  lifecycleState: "ready", generatedDeckId: "b".repeat(32), analysisPendingItems: 0,
} as unknown as DocumentSummary;
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function DocumentsRoute() {
  const jobs = useDocumentJobs();
  if (!jobs) throw new Error("Missing job owner");
  return <section aria-label="Documents route">
    <button onClick={() => { void jobs.generateStudyGuide(document); }}>Generate guide</button>
    <button onClick={() => { void jobs.stopStudyGuide(document); }}>Stop guide</button>
    <span role="status">{jobs.guideGeneratingDocumentIds.has(document.id) ? "Guide running" : "Idle"}</span>
    {jobs.errorMessage ? <p role="alert">{jobs.errorMessage}</p> : null}
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
  mocks.cancelGuide.mockResolvedValue(undefined);
  mocks.cancelDocument.mockResolvedValue(undefined);
  mocks.refreshDecks.mockResolvedValue(undefined);
  mocks.confirm.mockResolvedValue(true);
});

describe("application-lifetime document jobs", () => {
  it("retains a running guide across route unmount and completes elsewhere", async () => {
    const request = deferred<{ revision: number }>();
    mocks.guide.mockReturnValue(request.promise);
    open();
    await waitFor(() => expect(mocks.list).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Generate guide" }));
    expect(screen.getByRole("status").textContent).toBe("Guide running");
    fireEvent.click(screen.getByRole("button", { name: "Navigate" }));
    expect(screen.queryByRole("region", { name: "Documents route" })).toBeNull();
    expect(mocks.cancelGuide).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Navigate" }));
    expect(screen.getByRole("status").textContent).toBe("Guide running");
    fireEvent.click(screen.getByRole("button", { name: "Navigate" }));
    await act(async () => { request.resolve({ revision: 2 }); await request.promise; });
    await waitFor(() => expect(mocks.toast).toHaveBeenCalledWith("success", expect.stringContaining("revision 2")));
    fireEvent.click(screen.getByRole("button", { name: "Navigate" }));
    expect(screen.getByRole("status").textContent).toBe("Idle");
    expect(mocks.guide).toHaveBeenCalledTimes(1);
    expect(mocks.cancelGuide).not.toHaveBeenCalled();
  });
  it("does not erase a guide failure on refresh or navigation", async () => {
    const request = deferred<never>();
    mocks.guide.mockReturnValue(request.promise);
    open();
    fireEvent.click(screen.getByRole("button", { name: "Generate guide" }));
    fireEvent.click(screen.getByRole("button", { name: "Navigate" }));
    await act(async () => { request.reject(new Error("Guide contract failed")); await request.promise.catch(() => undefined); });
    fireEvent.click(screen.getByRole("button", { name: "Navigate" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Guide contract failed"));
    fireEvent.click(screen.getByRole("button", { name: "Refresh documents" }));
    await act(async () => { await Promise.resolve(); });
    expect(screen.getByRole("alert").textContent).toContain("Guide contract failed");
  });
  it("guards duplicate start and stop intents", async () => {
    const request = deferred<{ revision: number }>();
    mocks.guide.mockReturnValue(request.promise);
    open();
    const start = screen.getByRole("button", { name: "Generate guide" });
    fireEvent.click(start); fireEvent.click(start);
    expect(mocks.guide).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Stop guide" }));
    fireEvent.click(screen.getByRole("button", { name: "Stop guide" }));
    expect(mocks.cancelGuide).toHaveBeenCalledTimes(1);
    await act(async () => { request.resolve({ revision: 1 }); await request.promise; });
  });
  it("reports failed reconciliation without retrying a saved guide", async () => {
    mocks.guide.mockResolvedValue({ revision: 3 });
    mocks.refreshDecks.mockRejectedValue(new Error("Read failed"));
    open();
    fireEvent.click(screen.getByRole("button", { name: "Generate guide" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Saved successfully"));
    expect(mocks.guide).toHaveBeenCalledTimes(1);
    expect(mocks.toast).toHaveBeenCalledWith("success", expect.stringContaining("revision 3"));
  });
});

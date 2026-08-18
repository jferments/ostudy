import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DocumentGenerationProgress, DocumentSummary } from "../ipc/documents";
import { DocumentsScreen } from "./DocumentsScreen";

const listDocuments = vi.fn();
const getDocumentGenerationProgress = vi.fn();

vi.mock("../ipc/documents", async () => {
  const actual = await vi.importActual<typeof import("../ipc/documents")>("../ipc/documents");
  return {
    ...actual,
    listDocuments,
    getDocumentGenerationProgress,
  };
});

vi.mock("../platform/files", () => ({
  selectExternalImport: vi.fn().mockResolvedValue(null),
}));

vi.mock("../ui/overlays/overlayContext", () => ({
  useConfirm: () => vi.fn().mockResolvedValue(false),
}));

const document: DocumentSummary = {
  id: "0123456789abcdef0123456789abcdef",
  title: "Physiology",
  originalFilename: "physiology.pdf",
  byteSize: 3_600_000,
  sha256: "a".repeat(64),
  lifecycleState: "processing",
  pageCount: 55,
  extractedPageCount: 55,
  renderedPageCount: 55,
  extractionMode: "native",
  ocrPageCount: 0,
  ocrCompletedPageCount: 0,
  ocrMeanConfidence: null,
  ocrEngine: "",
  scaleMode: "large",
  estimatedSourceTokens: 80_000,
  outlineWindowCount: 10,
  outlineCompletedWindowCount: 3,
  generationWindowCount: 12,
  generationCompletedWindowCount: 0,
  generatedNoteCount: 0,
  generatedCardCount: 0,
  generatedDeckId: null,
  safeErrorCode: "",
  safeErrorMessage: "",
  createdAt: Date.now() - 60_000,
  updatedAt: Date.now(),
  completedAt: null,
  archivedAt: null,
  revision: 2,
  processingDurationMs: null,
  apiTokenCount: null,
  apiTokenUsageComplete: false,
};

const progress: DocumentGenerationProgress = {
  startedAt: Date.now() - 90_000,
  phase: "outline",
  currentWindow: 4,
  completedWindows: 3,
  totalWindows: 10,
  currentPageStart: 13,
  currentPageEnd: 18,
  currentPartition: 1,
  reasoningEffort: "low",
  providerRequestCount: 7,
  retryCount: 1,
  promptTokens: 12_000,
  completionTokens: 3_000,
  reasoningTokens: 2_000,
  totalTokens: 15_000,
  cachedPromptTokens: 6_000,
  cacheWriteTokens: 500,
  usageReportedRequests: 6,
  usageUnreportedRequests: 1,
};

describe("DocumentsScreen live generation progress", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listDocuments.mockResolvedValue([document]);
    getDocumentGenerationProgress.mockResolvedValue(progress);
  });

  it("shows stage, window progress, elapsed runtime, requests, retries, reasoning and cache usage", async () => {
    render(
      <DocumentsScreen
        refreshDecks={vi.fn().mockResolvedValue(undefined)}
        showToast={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getAllByText("Building outline 3/10").length).toBeGreaterThan(0));
    expect(screen.getByText("15k")).toBeTruthy();
    expect(screen.getByText("7")).toBeTruthy();
    expect(screen.getByText("1")).toBeTruthy();
    expect(screen.getByText("low · 2k")).toBeTruthy();
    expect(screen.getByText("12k / 3k")).toBeTruthy();
    expect(screen.getByText("6k (50.0%)")).toBeTruthy();
    expect(getDocumentGenerationProgress).toHaveBeenCalledWith(document.id);
  });
});

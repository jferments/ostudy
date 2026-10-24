import { describe, expect, it } from "vitest";
import type { DocumentGenerationProgress, DocumentSummary } from "../../ipc/documents";
import { generationPercent, processingPhaseLabel, processingStepLabel, readingDetail, timeLeft } from "./DocumentProgress";

function document(overrides: Partial<DocumentSummary> = {}): DocumentSummary {
  return {
    id: "a".repeat(32), title: "GI physiology", originalFilename: "gi.pdf", byteSize: 2048,
    sha256: "0".repeat(64), lifecycleState: "processing", sourceFormat: "pdf", sourceUnitKind: "page",
    sourceUnitCount: 55, pageCount: 55, extractedPageCount: 55, renderedPageCount: 55,
    extractionMode: "native", ocrPageCount: 0, ocrCompletedPageCount: 0, ocrMeanConfidence: null,
    ocrEngine: "", scaleMode: "standard", estimatedSourceTokens: 0, outlineWindowCount: 0,
    outlineCompletedWindowCount: 0, generationWindowCount: 0, generationCompletedWindowCount: 0,
    generatedNoteCount: 0, generatedCardCount: 0, generatedDeckId: null, safeErrorCode: "",
    safeErrorMessage: "", createdAt: 0, updatedAt: 0, completedAt: null, revision: 1,
    processingDurationMs: null, apiTokenCount: null, apiTokenUsageComplete: false,
    ...overrides,
  };
}

function reading(completedWindows: number, overrides: Partial<DocumentGenerationProgress> = {}): DocumentGenerationProgress {
  return {
    startedAt: 0, phase: "understanding material", phaseStartedAt: 0, currentWindow: null,
    completedWindows, totalWindows: 592, currentPageStart: 20, currentPageEnd: 23,
    currentPartition: null, providerKind: "openrouter", modelId: "deepseek", requestedReasoningEffort: null,
    reasoningEffort: "none", currentRequestStartedAt: null, currentRequestStage: null,
    currentRequestBytes: null, currentRequestTextBytes: null, currentRequestImageCount: null,
    currentRequestEvidenceBytes: null, currentRequestMaxOutputTokens: null, providerRequestCount: 0,
    retryCount: 0, promptTokens: 0, completionTokens: 0, reasoningTokens: 0, totalTokens: 0,
    cachedPromptTokens: 0, cacheWriteTokens: 0, usageReportedRequests: 0, usageUnreportedRequests: 0,
    providerActivity: "streaming", lastProviderActivityAt: null, lastModelActivityAt: null,
    ...overrides,
  };
}

describe("reading progress in plain words", () => {
  it("names pages, not the internal pieces being read", () => {
    expect(readingDetail(document(), reading(112))).toBe("Pages 20–23 of 55");
    expect(readingDetail(document(), reading(112, { currentPageStart: 9, currentPageEnd: 9 }))).toBe("Page 9 of 55");
    expect(readingDetail(document(), reading(0, { currentPageStart: null, currentPageEnd: null }))).toBe("55 pages");
    expect(readingDetail(document({ sourceUnitKind: "slide", sourceUnitCount: 12 }), reading(3, { currentPageStart: 4, currentPageEnd: null }))).toBe("Slide 4 of 12");
  });
  it("fills the bar from the passages read, after page preparation", () => {
    expect(generationPercent(document(), reading(0))).toBe(20);
    expect(generationPercent(document(), reading(296))).toBe(58);
    expect(generationPercent(document(), reading(592))).toBe(95);
  });

  it("says what is happening, how far it got and roughly how long is left", () => {
    // 296 of 592 read in four minutes: about four more.
    expect(processingPhaseLabel(document(), false, reading(296), 240_000))
      .toBe("Reading your document · 50% read · about 4 min left");
    expect(processingPhaseLabel(document(), false, reading(560), 240_000))
      .toBe("Reading your document · 94% read · less than a minute left");
    expect(processingStepLabel(document(), false, reading(296))).toBe("Reading your document");
  });

  it("leaves out an estimate until there is enough pace to go on", () => {
    expect(processingPhaseLabel(document(), false, reading(10), 240_000)).toBe("Reading your document · 1% read");
    expect(timeLeft(0.5, 0, 10_000)).toBeNull();
    expect(timeLeft(0.25, 0, 3_600_000)).toBe("about 3 hr 0 min left");
  });

  it("keeps page preparation as its own step", () => {
    const preparing = document({ renderedPageCount: 11 });
    expect(processingPhaseLabel(preparing, false, reading(0), 240_000)).toBe("Preparing pages · 11 of 55");
    expect(generationPercent(preparing, reading(0))).toBe(4);
  });
});

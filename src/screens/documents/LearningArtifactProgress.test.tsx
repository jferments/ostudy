import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { DocumentGenerationProgress, DocumentSummary } from "../../ipc/documents";
import type { DocumentArtifactJob } from "../../ipc/studyGuides";
import {
  flashcardProgress, guideProgress, guideUsageDetail, LearningArtifactProgressPanel,
} from "./LearningArtifactProgress";

const document = {
  lifecycleState: "ready", generatedDeckId: "b".repeat(32),
  generatedCardCount: 30, generationWindowCount: 10, generationCompletedWindowCount: 10,
} as DocumentSummary;
const job: DocumentArtifactJob = {
  id: "c".repeat(32), documentId: "a".repeat(32), planId: null, artifactKind: "study_guide",
  state: "generating", totalUnits: 10, completedUnits: 4, safeErrorCode: "", safeErrorMessage: "",
  apiTokenCount: null, apiTokenUsageComplete: false,
  createdAt: 1, startedAt: 2, updatedAt: 3, completedAt: null,
};

describe("durable learning-artifact progress", () => {
  it("shows separate stacked bars while the guide runs after cards are saved", () => {
    render(<LearningArtifactProgressPanel document={document} progress={null} guideJob={job}
      guideStarting={true} readFailed={false} />);
    const cards = screen.getByRole("progressbar", { name: "Flashcards generation progress" });
    const guide = screen.getByRole("progressbar", { name: "Study Guide generation progress" });
    expect(cards.getAttribute("aria-valuenow")).toBe("100");
    expect(guide.getAttribute("aria-valuenow")).toBe("40");
    expect(cards.compareDocumentPosition(guide) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
    expect(screen.getAllByText("Complete")).toHaveLength(1);
    expect(screen.getByText("4 / 10 sections completed")).toBeTruthy();
  });

  it("counts finished sections, then says the guide is being organized", () => {
    expect(guideProgress({ ...job, completedUnits: 9 }, true).status).toBe("9 / 10 sections completed");
    expect(guideProgress({ ...job, completedUnits: 10 }, true).status).toBe("Organizing the guide");
  });

  it("shows provider-reported study-guide token usage while generation is active", () => {
    const reporting = { ...job, apiTokenCount: 12_345 };
    render(<LearningArtifactProgressPanel document={document} progress={null} guideJob={reporting}
      guideStarting={true} readFailed={false} />);
    expect(screen.getByText("Study Guide usage · 12,345 reported tokens so far")).toBeTruthy();
    expect(guideUsageDetail({ ...job, apiTokenCount: null }))
      .toBe("Study Guide usage · Awaiting provider usage");
  });

  it("distinguishes complete from partial terminal provider usage", () => {
    expect(guideUsageDetail({
      ...job, state: "succeeded", apiTokenCount: 20_000, apiTokenUsageComplete: true,
    })).toBe("Study Guide usage · 20,000 tokens");
    expect(guideUsageDetail({
      ...job, state: "failed", apiTokenCount: 15_000, apiTokenUsageComplete: false,
    })).toBe("Study Guide usage · 15,000+ reported tokens");
  });

  it("does not call a completed request count a published artifact", () => {
    expect(guideProgress({ ...job, state: "committing", completedUnits: 10 }, false))
      .toEqual({ value: 100, status: "Saving guide", complete: false, failed: false });
    expect(flashcardProgress({ ...document, lifecycleState: "processing" }, null).complete).toBe(false);
  });

  it("retains failed progress instead of pretending that the guide completed", () => {
    const failed = { ...job, state: "failed" as const, safeErrorMessage: "Provider unavailable" };
    render(<LearningArtifactProgressPanel document={document} progress={null} guideJob={failed}
      guideStarting={false} readFailed={false} />);
    expect(screen.getByText("Needs attention")).toBeTruthy();
    expect(screen.getByText("Provider unavailable")).toBeTruthy();
    expect(screen.getAllByText("Complete")).toHaveLength(1);
    expect(screen.getByRole("progressbar", { name: "Study Guide generation progress" }).getAttribute("aria-valuenow")).toBe("40");
  });

  it("shows completion for each independently published artifact", () => {
    render(<LearningArtifactProgressPanel document={document} progress={null}
      guideJob={{ ...job, state: "succeeded", completedUnits: 10, completedAt: 9 }}
      guideStarting={false} readFailed={false} />);
    expect(screen.getAllByText("Complete")).toHaveLength(2);
    expect(screen.getAllByRole("progressbar").map((bar) => bar.getAttribute("aria-valuenow"))).toEqual(["100", "100"]);
  });

  it("keeps flashcards running when the sibling guide finishes first", () => {
    const processing = {
      ...document, lifecycleState: "processing" as const, generatedDeckId: null,
      generatedCardCount: 0, generationWindowCount: 15, generationCompletedWindowCount: 12,
      safeErrorCode: "", safeErrorMessage: "",
    };
    render(<LearningArtifactProgressPanel document={processing} progress={null}
      guideJob={{ ...job, state: "succeeded", completedUnits: 10, completedAt: 9 }}
      guideStarting={false} readFailed={false} />);
    expect(screen.getByText("12 / 15 windows")).toBeTruthy();
    expect(screen.queryByText("Needs attention")).toBeNull();
    expect(screen.getAllByText("Complete")).toHaveLength(1);
    expect(screen.getAllByRole("progressbar").map((bar) => bar.getAttribute("aria-valuenow"))).toEqual(["80", "100"]);
  });

  it("shows a terminal card error even when a published guide keeps the document ready", () => {
    const failedCards = {
      ...document, generatedDeckId: null, generatedCardCount: 0,
      generationWindowCount: 15, generationCompletedWindowCount: 12,
      safeErrorCode: "ai_document_output_invalid", safeErrorMessage: "Card validation failed",
    };
    render(<LearningArtifactProgressPanel document={failedCards} progress={null}
      guideJob={{ ...job, state: "succeeded", completedUnits: 10, completedAt: 9 }}
      guideStarting={false} readFailed={false} />);
    expect(screen.getByText("Needs attention")).toBeTruthy();
    expect(screen.queryByText("12 / 15 windows")).toBeNull();
    expect(screen.getAllByText("Complete")).toHaveLength(1);
    expect(screen.getAllByRole("progressbar").map((bar) => bar.getAttribute("aria-valuenow"))).toEqual(["80", "100"]);
  });

  it.each(["document_generation_canceled", "ai_request_canceled"])(
    "shows stopped cards on a ready guide-only document for %s", (safeErrorCode) => {
      expect(flashcardProgress({ ...document, generatedDeckId: null, safeErrorCode }, null))
        .toEqual({ value: 100, status: "Stopped", complete: false, failed: true });
    },
  );

  it("does not mistake a stale error for the result of an active retry", () => {
    expect(flashcardProgress({
      ...document, lifecycleState: "processing", generatedDeckId: null,
      safeErrorCode: "ai_document_output_invalid",
    }, null).failed).toBe(false);
  });

  it("says what the document is doing before cards or the guide can start", () => {
    const planning = {
      ...document, lifecycleState: "processing", generatedDeckId: null, sourceUnitCount: 55,
      sourceUnitKind: "page", sourceFormat: "pdf", renderedPageCount: 55,
      outlineWindowCount: 40, outlineCompletedWindowCount: 12,
      generationWindowCount: 58, generationCompletedWindowCount: 0,
    } as DocumentSummary;
    const live = {
      phase: "outline", currentWindow: 13, currentWindowEnd: 20, currentPageStart: 14, currentPageEnd: 19,
      completedWindows: 12, totalWindows: 40,
    } as DocumentGenerationProgress;
    render(<LearningArtifactProgressPanel document={planning} progress={live}
      guideJob={{ ...job, state: "preparing", completedUnits: 0 }} guideStarting={true} readFailed={false} />);
    expect(screen.getByText("Planning the study material")).toBeTruthy();
    expect(screen.getByText("Active windows 13–20 of 40 · pages 14–19.")).toBeTruthy();
    expect(screen.getByText("Waiting")).toBeTruthy();
    expect(screen.queryByText(/58 windows/)).toBeNull();
    expect(screen.queryByText(/Awaiting provider usage/)).toBeNull();
  });

  it("drops the step line once cards are being written", () => {
    const writing = { ...document, lifecycleState: "processing", generatedDeckId: null,
      generationCompletedWindowCount: 3 } as DocumentSummary;
    const live = { phase: "generating", currentWindow: 4, currentWindowEnd: null,
      currentPageStart: null, currentPageEnd: null } as DocumentGenerationProgress;
    render(<LearningArtifactProgressPanel document={writing} progress={live} guideJob={null}
      guideStarting={false} readFailed={false} />);
    expect(screen.queryByTestId("document-current-step")).toBeNull();
    expect(screen.getByText("3 / 10 windows")).toBeTruthy();
  });
});

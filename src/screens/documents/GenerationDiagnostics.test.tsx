import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { DocumentGenerationProgress } from "../../ipc/documents";
import { GenerationDiagnostics } from "./DocumentProgress";

function progress(overrides: Partial<DocumentGenerationProgress> = {}): DocumentGenerationProgress {
  return {
    startedAt: 1000, phase: "generating", phaseStartedAt: 61000, currentWindow: 1,
    completedWindows: 0, totalWindows: 2, currentPageStart: 1, currentPageEnd: 4,
    currentPartition: null, providerKind: "openrouter",
    modelId: "deepseek/deepseek-v4.1-flash", requestedReasoningEffort: "automatic",
    reasoningEffort: "none", currentRequestStartedAt: 81000,
    currentRequestStage: "flashcard_authoring", currentRequestBytes: 65536,
    currentRequestTextBytes: 16384, currentRequestImageCount: 4,
    currentRequestEvidenceBytes: 8192, currentRequestMaxOutputTokens: 6144,
    providerRequestCount: 5, retryCount: 2,
    transientRetryCount: 1, compatibilityRetryCount: 0, contractRepairRetryCount: 1,
    outputCompactionRetryCount: 0, promptTokens: 12000, completionTokens: 3000,
    reasoningTokens: 2000, totalTokens: 15000, cachedPromptTokens: 6000,
    cacheWriteTokens: 500, usageReportedRequests: 5, usageUnreportedRequests: 0,
    providerActivity: "streaming", lastProviderActivityAt: 90000, lastModelActivityAt: 90000,
    ...overrides,
  };
}
function stat(label: string) { return screen.getByText(label).nextElementSibling?.textContent; }

describe("generation cost summary and optional retry details", () => {
  it("keeps time, requests, retry total and tokens outside the collapsed disclosure", () => {
    render(<GenerationDiagnostics progress={progress()} now={91000} />);
    const trigger = screen.getByRole("button", { name: "Provider and request" });
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(stat("Elapsed")).toBe("1m 30s");
    expect(stat("Stage elapsed")).toBe("30 sec");
    expect(stat("Requests")).toBe("5");
    expect(stat("Retries")).toBe("2");
    expect(stat("Reasoning")).toBe("automatic → none · 2.0k");
    expect(stat("Prompt / output")).toBe("12k / 3.0k");
    expect(stat("Prompt cache")).toBe("6.0k (50.0%)");
    expect(screen.getByText("12k / 3.0k").closest('[data-slot="collapsible"]')).toBeNull();
    fireEvent.click(trigger);
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByText("Provider compatibility")).toBeTruthy();
    expect(screen.getByText("Response contract repair")).toBeTruthy();
    expect(screen.getByText("Temporary transport failure")).toBeTruthy();
    expect(stat("Provider")).toBe("openrouter");
    expect(stat("Model")).toBe("deepseek/deepseek-v4.1-flash");
    expect(stat("Requested reasoning")).toBe("automatic");
    expect(stat("Effective reasoning")).toBe("none");
    expect(stat("Cache writes")).toBe("500 tokens");
    expect(stat("Stage")).toBe("Flashcard Authoring");
    expect(stat("Request elapsed")).toBe("10 sec");
    expect(stat("Wire size")).toBe("64 KB");
    expect(stat("Text/schema")).toBe("16 KB");
    expect(stat("Evidence payload")).toBe("8.0 KB");
    expect(stat("Rendered images")).toBe("4");
    expect(stat("Output ceiling")).toBe("6,144 tokens");
    fireEvent.click(trigger);
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(stat("Retries")).toBe("2");
  });

  it("does not turn absent provider usage into zero tokens", () => {
    render(<GenerationDiagnostics progress={progress({
      promptTokens: 0, completionTokens: 0, reasoningTokens: 0, totalTokens: 0,
      cachedPromptTokens: 0, cacheWriteTokens: 0, usageReportedRequests: 0,
    })} now={91000} />);
    expect(stat("Prompt / output")).toBe("Awaiting usage");
    expect(stat("Prompt cache")).toBe("Awaiting usage");
    expect(stat("Reasoning")).toBe("automatic → none · Awaiting usage");
    expect(screen.queryByText("0 / 0")).toBeNull();
  });

  it("preserves explicit reported zero and discloses partial accounting", () => {
    render(<GenerationDiagnostics progress={progress({
      promptTokens: 0, completionTokens: 0, reasoningTokens: 0, totalTokens: 0,
      cachedPromptTokens: 0, cacheWriteTokens: 0, usageReportedRequests: 1,
      usageUnreportedRequests: 4,
    })} now={91000} />);
    expect(stat("Prompt / output")).toBe("0 / 0");
    expect(stat("Prompt cache")).toBe("0");
    expect(stat("Reasoning")).toBe("automatic → none · 0");
    expect(screen.getByText(/Usage incomplete: 4 requests/u)).toBeTruthy();
    expect(screen.queryByText(/NaN/u)).toBeNull();
  });


  it("does not invent current request measurements after an attempt finishes", () => {
    render(<GenerationDiagnostics progress={progress({
      currentRequestStartedAt: null,
      currentRequestStage: null,
      currentRequestBytes: null,
      currentRequestTextBytes: null,
      currentRequestImageCount: null,
      currentRequestEvidenceBytes: null,
      currentRequestMaxOutputTokens: null,
    })} now={91000} />);
    fireEvent.click(screen.getByRole("button", { name: "Provider and request" }));
    expect(screen.queryByText("Current provider request")).toBeNull();
    expect(stat("Provider")).toBe("openrouter");
    expect(stat("Requested reasoning")).toBe("automatic");
    expect(stat("Effective reasoning")).toBe("none");
  });

  it("does not invent details for a missing progress snapshot", () => {
    render(<GenerationDiagnostics progress={null} now={91000} />);
    expect(stat("Elapsed")).toBe("Starting…");
    expect(stat("Retries")).toBe("—");
    expect(stat("Prompt / output")).toBe("Awaiting usage");
    expect(screen.queryByRole("button", { name: "Provider and request" })).toBeNull();
  });
});

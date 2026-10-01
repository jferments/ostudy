import { beforeEach, describe, expect, it, vi } from "vitest";

import { importDocumentAndGenerateFlashcards, retryDocumentFlashcardGeneration,
  startDocumentFlashcardGeneration } from "./documents";
import { generationInstructionsInput, MAX_GENERATION_INSTRUCTIONS_BYTES } from "./generationInstructions";

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("./transport", () => ({ invoke: mocks.invoke }));
const documentId = "a".repeat(32);
beforeEach(() => { mocks.invoke.mockReset().mockResolvedValue(undefined); });

describe("per-generation learner instructions", () => {
  it("keeps the existing blank/omitted request shape and normalizes line breaks once", async () => {
    for (const value of [undefined, null, "", " \r\n\t "]) {
      expect(generationInstructionsInput(value)).toEqual({});
    }
    expect(generationInstructionsInput("  First line\r\nSecond line\rThird line  "))
      .toEqual({ generationInstructions: "First line\nSecond line\nThird line" });
    await retryDocumentFlashcardGeneration(documentId);
    expect(mocks.invoke).toHaveBeenLastCalledWith("run_document_flashcard_generation", { input: { documentId } });
    await startDocumentFlashcardGeneration(documentId, " \r\n");
    expect(mocks.invoke).toHaveBeenLastCalledWith("start_document_flashcard_generation", { input: { documentId } });
  });

  it("carries guidance into both native initial-generation launch paths without a global setting write", async () => {
    const text = "For each cranial nerve, ask how a nurse assesses its function.";
    await startDocumentFlashcardGeneration(documentId, `  ${text}  `);
    expect(mocks.invoke).toHaveBeenLastCalledWith("start_document_flashcard_generation", {
      input: { documentId, generationInstructions: text },
    });
    await retryDocumentFlashcardGeneration(documentId, text);
    expect(mocks.invoke).toHaveBeenLastCalledWith("run_document_flashcard_generation", {
      input: { documentId, generationInstructions: text },
    });
    expect(mocks.invoke).toHaveBeenCalledTimes(2);
  });

  it("captures independent requests and sends instructions only after the managed document exists", async () => {
    mocks.invoke.mockResolvedValueOnce({ documentId });
    await importDocumentAndGenerateFlashcards("b".repeat(32), "Nursing assessments");
    expect(mocks.invoke.mock.calls).toEqual([
      ["import_document", { input: { stagedFileId: "b".repeat(32) } }],
      ["run_document_flashcard_generation", { input: { documentId, generationInstructions: "Nursing assessments" } }],
    ]);
    await startDocumentFlashcardGeneration("c".repeat(32), "Arterial anatomy");
    expect(mocks.invoke.mock.calls[1]?.[1]).toEqual({ input: { documentId, generationInstructions: "Nursing assessments" } });
    expect(mocks.invoke).toHaveBeenLastCalledWith("start_document_flashcard_generation", {
      input: { documentId: "c".repeat(32), generationInstructions: "Arterial anatomy" },
    });
  });

  it("rejects oversized UTF-8 and control characters before any source or task mutation", async () => {
    const exact = "🧠".repeat(MAX_GENERATION_INSTRUCTIONS_BYTES / 4);
    expect(generationInstructionsInput(exact).generationInstructions).toBe(exact);
    expect(() => generationInstructionsInput(`${exact}🧠`)).toThrow("16 KiB");
    expect(() => generationInstructionsInput("private\0prompt")).toThrow("control characters");
    const oversized = "x".repeat(MAX_GENERATION_INSTRUCTIONS_BYTES + 1);
    await expect(importDocumentAndGenerateFlashcards("b".repeat(32), oversized)).rejects.toThrow("not been truncated");
    await expect(startDocumentFlashcardGeneration(documentId, oversized)).rejects.toThrow("16 KiB");
    await expect(retryDocumentFlashcardGeneration(documentId, "private\u0085prompt")).rejects.toThrow("control characters");
    expect(mocks.invoke).not.toHaveBeenCalled();
  });
});

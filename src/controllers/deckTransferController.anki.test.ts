import { describe, expect, it, vi } from "vitest";

import type { AnkiImportReport, DeckSummary } from "../ipc/decks";
import type { StagedExternalFile } from "../ipc/externalFiles";
import { DeckTransferController, type DeckTransferDependencies } from "./deckTransferController";

const deck: DeckSummary = {
  id: "a".repeat(32), name: "Examples", description: "", sourceNoteCount: 6,
  generatedCardCount: 6, studyableDueCount: 6, archivedAt: null,
  createdAt: 1, updatedAt: 1, revision: 1,
};
const selected: StagedExternalFile = {
  id: "b".repeat(32), purpose: "study_content", detectedKind: "anki_apkg",
  displayName: "examples.apkg", mediaType: "application/octet-stream",
  byteSize: 2048, sha256: "c".repeat(64), expiresAt: 100,
};
const report: AnkiImportReport = {
  sourceCardCount: 7, importedCardCount: 6, skippedCardCount: 1,
  simplifiedCardCount: 0, nativeStyleCardCount: 3, typedAnswerCardCount: 1,
  nativeClozeCardCount: 2, clozeFallbackCardCount: 0, omittedIssueCount: 0,
  issues: [{ sourceCardId: "105", message: "Skipped: JavaScript-dependent card." }],
};

function harness() {
  const native = {
    prepareExport: vi.fn(), importStudydeck: vi.fn(),
    importGenerated: vi.fn(), invalidate: vi.fn(),
  };
  const dependencies: DeckTransferDependencies = {
    selectImport: vi.fn().mockResolvedValue(selected),
    publish: vi.fn(), share: vi.fn(), createDecksController: () => native,
    createOperationId: vi.fn(() => "d".repeat(32)),
  };
  return { native, dependencies, controller: new DeckTransferController(dependencies) };
}

describe("Anki deck transfer", () => {
  it("routes the opaque APKG selection through native import and preserves the report", async () => {
    const { native, controller } = harness();
    native.importStudydeck.mockResolvedValue({ deck, cardCount: 6, assetCount: 1, ankiReport: report });
    const outcome = await controller.importFromFile();
    expect(native.importStudydeck).toHaveBeenCalledWith(selected.id, "d".repeat(32));
    expect(native.importGenerated).not.toHaveBeenCalled();
    expect(outcome?.ankiReport).toEqual(report);
    expect(outcome?.committedDeck).toEqual(deck);
    expect(outcome?.message).toContain("1 Anki cards skipped");
    expect(outcome?.message).toContain("typed-answer cards converted");
    expect(outcome?.message).toContain("cloze cards converted to native Fill in the blank");
  });

  it("retains attempt identity after an ambiguous transport failure", async () => {
    const { native, dependencies, controller } = harness();
    native.importStudydeck.mockRejectedValueOnce(new Error("connection lost"))
      .mockResolvedValueOnce({ deck, cardCount: 6, assetCount: 1, ankiReport: report });
    await expect(controller.importFromFile()).rejects.toThrow("connection lost");
    await controller.importFromFile();
    expect(native.importStudydeck).toHaveBeenNthCalledWith(2, selected.id, "d".repeat(32));
    expect(dependencies.createOperationId).toHaveBeenCalledOnce();
  });

  it("returns committed success when receipt reconciliation reports a warning", async () => {
    const { native, controller } = harness();
    native.importStudydeck.mockResolvedValue({
      deck, cardCount: 6, assetCount: 1, ankiReport: report,
      postCommitWarning: "The deck was imported, but its transfer receipt could not be saved.",
    });
    const outcome = await controller.importFromFile();
    expect(outcome?.deckId).toBe(deck.id);
    expect(outcome?.message).toContain("transfer receipt could not be saved");
  });

  it("propagates actionable format rejection without inventing an imported deck", async () => {
    const { native, controller } = harness();
    native.importStudydeck.mockRejectedValue({
      code: "invalid_anki_package", message: "Export with Support older Anki versions enabled.",
    });
    await expect(controller.importFromFile()).rejects.toMatchObject({ code: "invalid_anki_package" });
    expect(native.importGenerated).not.toHaveBeenCalled();
  });
});

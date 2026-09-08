import { describe, expect, it, vi } from "vitest";

import type { AnkiImportReport, DeckSummary } from "../ipc/decks";
import type { StagedExternalFile } from "../ipc/externalFiles";
import type { OperationProgress } from "../ipc/operationProgress";
import { DeckTransferController, type DeckTransferDependencies } from "./deckTransferController";

const deck: DeckSummary = {
  id: "a".repeat(32), name: "Examples", description: "", sourceNoteCount: 7,
  generatedCardCount: 7, studyableDueCount: 7, archivedAt: null,
  createdAt: 1, updatedAt: 1, revision: 1,
};
const selected: StagedExternalFile = {
  id: "b".repeat(32), purpose: "study_content", detectedKind: "anki_apkg",
  displayName: "examples.apkg", mediaType: "application/octet-stream",
  byteSize: 2048, sha256: "c".repeat(64), expiresAt: 100,
};
const report: AnkiImportReport = {
  sourceCardCount: 8, importedCardCount: 7, skippedCardCount: 1,
  simplifiedCardCount: 1, nativeStyleCardCount: 3, typedAnswerCardCount: 1,
  nativeClozeCardCount: 2, clozeFallbackCardCount: 0, javascriptSalvagedCardCount: 1,
  omittedIssueCount: 0,
  issues: [{ sourceCardId: "107", message: "Skipped: JavaScript could not be reduced to static content." }],
};

function harness() {
  const native = {
    prepareExport: vi.fn(), importStudydeck: vi.fn(),
    importGenerated: vi.fn(), invalidate: vi.fn(),
  };
  const unlisten = vi.fn();
  const listenProgress = vi.fn().mockResolvedValue(unlisten);
  const dependencies: DeckTransferDependencies = {
    selectImport: vi.fn().mockResolvedValue(selected),
    publish: vi.fn(), share: vi.fn(), createDecksController: () => native,
    createOperationId: vi.fn(() => "d".repeat(32)),
    listenProgress,
  };
  return {
    native,
    unlisten,
    listenProgress,
    dependencies,
    controller: new DeckTransferController(dependencies),
  };
}

describe("Anki deck transfer", () => {
  it("routes the opaque APKG selection through native import and preserves the report", async () => {
    const { native, controller } = harness();
    native.importStudydeck.mockResolvedValue({ deck, cardCount: 7, assetCount: 1, ankiReport: report });
    const outcome = await controller.importFromFile();
    expect(native.importStudydeck).toHaveBeenCalledWith(selected.id, "d".repeat(32));
    expect(native.importGenerated).not.toHaveBeenCalled();
    expect(outcome?.ankiReport).toEqual(report);
    expect(outcome?.committedDeck).toEqual(deck);
    expect(outcome?.message).toContain("1 Anki cards skipped");
    expect(outcome?.message).toContain("typed-answer cards converted");
    expect(outcome?.message).toContain("cloze cards converted to native Fill in the blank");
    expect(outcome?.message).toContain("JavaScript cards recovered as static content");
  });

  it("subscribes to operation progress before importing and always unregisters", async () => {
    const { native, listenProgress, unlisten, controller } = harness();
    const progress: OperationProgress = {
      schemaVersion: 1,
      operationId: "d".repeat(32),
      operationKind: "anki_import",
      phase: "converting_cards",
      processed: 100,
      total: 800,
      succeeded: 98,
      failed: 2,
    };
    listenProgress.mockImplementationOnce(async (_operationId, listener) => {
      listener(progress);
      return unlisten;
    });
    native.importStudydeck.mockResolvedValue({ deck, cardCount: 7, assetCount: 1, ankiReport: report });
    const observed: OperationProgress[] = [];
    await controller.importFromFile((update) => observed.push(update));
    expect(listenProgress).toHaveBeenCalledWith("d".repeat(32), expect.any(Function));
    expect(listenProgress.mock.invocationCallOrder[0]).toBeLessThan(
      native.importStudydeck.mock.invocationCallOrder[0] ?? Number.MAX_SAFE_INTEGER,
    );
    expect(observed).toEqual([progress]);
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("retains attempt identity after an ambiguous transport failure", async () => {
    const { native, dependencies, controller } = harness();
    native.importStudydeck.mockRejectedValueOnce(new Error("connection lost"))
      .mockResolvedValueOnce({ deck, cardCount: 7, assetCount: 1, ankiReport: report });
    await expect(controller.importFromFile()).rejects.toThrow("connection lost");
    await controller.importFromFile();
    expect(native.importStudydeck).toHaveBeenNthCalledWith(2, selected.id, "d".repeat(32));
    expect(dependencies.createOperationId).toHaveBeenCalledOnce();
  });

  it("returns committed success when receipt reconciliation reports a warning", async () => {
    const { native, controller } = harness();
    native.importStudydeck.mockResolvedValue({
      deck, cardCount: 7, assetCount: 1, ankiReport: report,
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

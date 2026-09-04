import { describe, expect, it, vi } from "vitest";

import {
  DeckTransferController,
  type DeckTransferDependencies,
} from "./deckTransferController";
import type { DeckSummary } from "../ipc/decks";

function deck(seed: string, name: string): DeckSummary {
  return {
    id: seed.repeat(32),
    name,
    description: "",
    sourceNoteCount: 1,
    generatedCardCount: 4,
    studyableDueCount: 2,
    archivedAt: null,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  };
}

function harness() {
  const controller = {
    prepareExport: vi.fn(),
    importStudydeck: vi.fn(),
    importGenerated: vi.fn(),
    invalidate: vi.fn(),
  };
  const dependencies: DeckTransferDependencies = {
    selectImport: vi.fn(),
    publish: vi.fn(),
    share: vi.fn(),
    createDecksController: () => controller,
    createOperationId: vi.fn(() => "operation-1"),
  };
  return {
    controller,
    dependencies,
    transfer: new DeckTransferController(dependencies),
  };
}

describe("DeckTransferController", () => {
  it("treats a cancelled file selection as a no-op", async () => {
    const { dependencies, transfer } = harness();
    vi.mocked(dependencies.selectImport).mockResolvedValue(null);

    await expect(transfer.importFromFile()).resolves.toBeNull();
  });

  it("reuses the same idempotency key when a studydeck import is retried after an unclassified failure", async () => {
    const { controller, dependencies, transfer } = harness();
    const selected = {
      id: "staged-file",
      purpose: "study_content" as const,
      detectedKind: "studydeck" as const,
      displayName: "nursing.studydeck",
      mediaType: "application/octet-stream",
      byteSize: 10,
      sha256: "a".repeat(64),
      expiresAt: 100,
    };
    vi.mocked(dependencies.selectImport).mockResolvedValue(selected);
    controller.importStudydeck
      .mockRejectedValueOnce(new Error("transport interrupted"))
      .mockResolvedValueOnce({ deck: deck("a", "Nursing"), cardCount: 4 });

    await expect(transfer.importFromFile()).rejects.toThrow("transport interrupted");
    const outcome = await transfer.importFromFile();

    expect(controller.importStudydeck).toHaveBeenNthCalledWith(
      1,
      selected.id,
      "operation-1",
    );
    expect(controller.importStudydeck).toHaveBeenNthCalledWith(
      2,
      selected.id,
      "operation-1",
    );
    expect(dependencies.createOperationId).toHaveBeenCalledOnce();
    expect(outcome?.message).toBe("Imported “Nursing” with 4 cards.");
  });

  it("reports generated-card imports and duplicate skips", async () => {
    const { controller, dependencies, transfer } = harness();
    vi.mocked(dependencies.selectImport).mockResolvedValue({
      id: "generated-file",
      purpose: "study_content",
      detectedKind: "generated_json",
      displayName: "cards.json",
      mediaType: "application/json",
      byteSize: 10,
      sha256: "b".repeat(64),
      expiresAt: 100,
    });
    controller.importGenerated.mockResolvedValue({
      deckId: "d".repeat(32),
      createdNoteCount: 2,
      createdCardCount: 3,
      skippedDuplicateCount: 1,
      errors: [],
    });

    const outcome = await transfer.importFromFile();

    expect(outcome?.committedDeck).toBeNull();
    expect(outcome?.message).toContain("Imported 2 card entries as 3 scheduled cards.");
    expect(outcome?.message).toContain("1 exact duplicate was skipped.");
  });

  it("deduplicates selected decks and stops cleanly when a later save picker is cancelled", async () => {
    const { controller, dependencies, transfer } = harness();
    const anatomy = deck("a", "Anatomy");
    const pharmacology = deck("b", "Pharmacology");
    controller.prepareExport
      .mockResolvedValueOnce({
        preparedExport: { id: "prepared-a" },
        cardCount: 4,
      })
      .mockResolvedValueOnce({
        preparedExport: { id: "prepared-b" },
        cardCount: 4,
      });
    vi.mocked(dependencies.publish)
      .mockResolvedValueOnce({
        preparedExportId: "prepared-a",
        displayName: "Anatomy.studydeck",
        destinationKind: "filesystem",
        completedAt: 1,
      })
      .mockResolvedValueOnce(null);

    const outcome = await transfer.exportDecks([anatomy, anatomy, pharmacology]);

    expect(outcome).toMatchObject({
      requestedCount: 2,
      cancelled: true,
    });
    expect(outcome.exported.map(({ deck: exportedDeck }) => exportedDeck.id)).toEqual([
      anatomy.id,
    ]);
    expect(controller.prepareExport).toHaveBeenCalledTimes(2);
  });

  it("wraps export failures with the affected deck name", async () => {
    const { controller, transfer } = harness();
    const anatomy = deck("a", "Anatomy");
    controller.prepareExport.mockRejectedValue(new Error("disk unavailable"));

    await expect(transfer.exportDecks([anatomy])).rejects.toThrow(
      "Could not export “Anatomy”: disk unavailable",
    );
  });

  it("returns a share outcome only when the platform launches sharing", async () => {
    const { controller, dependencies, transfer } = harness();
    const anatomy = deck("a", "Anatomy");
    controller.prepareExport.mockResolvedValue({
      preparedExport: { id: "prepared-a" },
      cardCount: 4,
    });
    vi.mocked(dependencies.share).mockResolvedValue({
      preparedExportId: "prepared-a",
      launched: true,
      destinationKind: "system_share",
      launchedAt: 1,
    });

    const outcome = await transfer.shareDeck(anatomy);

    expect(outcome?.deck.id).toBe(anatomy.id);
    expect(outcome?.cardCount).toBe(4);
    transfer.invalidate();
    expect(controller.invalidate).toHaveBeenCalledOnce();
  });
});

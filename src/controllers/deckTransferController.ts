import { createOperationId, getErrorCode, getErrorMessage } from "../ipc/common";
import type { AnkiImportReport, DeckSummary } from "../ipc/decks";
import type { PublishReceipt, ShareLaunch, StagedExternalFile } from "../ipc/externalFiles";
import {
  publishExport,
  selectExternalImport,
  shareExport,
} from "../platform/files";
import { DecksController } from "./decksController";

export interface DeckImportOutcome {
  deckId: string;
  committedDeck: DeckSummary | null;
  message: string;
  ankiReport?: AnkiImportReport;
}

export interface DeckExportOutcome {
  deck: DeckSummary;
  cardCount: number;
  receipt: PublishReceipt;
}

export interface DeckShareOutcome {
  deck: DeckSummary;
  cardCount: number;
  launch: ShareLaunch;
}

export interface BatchDeckExportOutcome {
  requestedCount: number;
  exported: DeckExportOutcome[];
  cancelled: boolean;
}

export interface DeckTransferDependencies {
  selectImport(): Promise<StagedExternalFile | null>;
  publish(preparedExportId: string): Promise<PublishReceipt | null>;
  share(preparedExportId: string): Promise<ShareLaunch>;
  createDecksController(): Pick<
    DecksController,
    "prepareExport" | "importStudydeck" | "importGenerated" | "invalidate"
  >;
  createOperationId(): string;
}

const defaultDependencies: DeckTransferDependencies = {
  selectImport: () => selectExternalImport("study_content"),
  publish: publishExport,
  share: shareExport,
  createDecksController: () => new DecksController(),
  createOperationId,
};

/**
 * Owns portable deck-file orchestration independently from the screen or dialog
 * that launches it. This keeps file staging, idempotent import identity, export
 * preparation, and platform publication out of presentational components.
 */
export class DeckTransferController {
  readonly #dependencies: DeckTransferDependencies;
  readonly #decks: ReturnType<DeckTransferDependencies["createDecksController"]>;
  #pendingStudydeckImport: { stagedFileId: string; operationId: string } | null = null;

  constructor(dependencies: DeckTransferDependencies = defaultDependencies) {
    this.#dependencies = dependencies;
    this.#decks = dependencies.createDecksController();
  }

  async importFromFile(): Promise<DeckImportOutcome | null> {
    const selected = await this.#dependencies.selectImport();
    if (!selected) return null;

    if (selected.detectedKind === "studydeck" || selected.detectedKind === "anki_apkg") {
      const pending =
        this.#pendingStudydeckImport?.stagedFileId === selected.id
          ? this.#pendingStudydeckImport
          : {
              stagedFileId: selected.id,
              operationId: this.#dependencies.createOperationId(),
            };
      this.#pendingStudydeckImport = pending;
      try {
        const result = await this.#decks.importStudydeck(selected.id, pending.operationId);
        this.#pendingStudydeckImport = null;
        const compatibility = result.ankiReport
          ? ankiCompatibilitySummary(result.ankiReport)
          : "";
        const warning = result.postCommitWarning ? ` ${result.postCommitWarning}` : "";
        return {
          deckId: result.deck.id,
          committedDeck: result.deck,
          message: `Imported “${result.deck.name}” with ${result.cardCount} ${
            result.cardCount === 1 ? "card" : "cards"
          }.${compatibility}${warning}`,
          ...(result.ankiReport ? { ankiReport: result.ankiReport } : {}),
        };
      } catch (error: unknown) {
        if (getErrorCode(error) !== null) this.#pendingStudydeckImport = null;
        throw error;
      }
    }

    if (selected.detectedKind === "generated_json") {
      this.#pendingStudydeckImport = null;
      const result = await this.#decks.importGenerated(selected.id);
      const duplicateMessage =
        result.skippedDuplicateCount > 0
          ? ` ${result.skippedDuplicateCount} exact ${
              result.skippedDuplicateCount === 1 ? "duplicate was" : "duplicates were"
            } skipped.`
          : "";
      return {
        deckId: result.deckId,
        committedDeck: null,
        message: `Imported ${result.createdNoteCount} ${
          result.createdNoteCount === 1 ? "card entry" : "card entries"
        } as ${result.createdCardCount} ${
          result.createdCardCount === 1 ? "scheduled card" : "scheduled cards"
        }.${duplicateMessage}`,
      };
    }

    this.#pendingStudydeckImport = null;
    throw new Error("Select an ostudy deck, an Anki .apkg deck, or a generated-card file.");
  }

  async exportDeck(deck: DeckSummary): Promise<DeckExportOutcome | null> {
    const prepared = await this.#decks.prepareExport(
      deck.id,
      this.#dependencies.createOperationId(),
    );
    const receipt = await this.#dependencies.publish(prepared.preparedExport.id);
    return receipt ? { deck, cardCount: prepared.cardCount, receipt } : null;
  }

  async exportDecks(decks: readonly DeckSummary[]): Promise<BatchDeckExportOutcome> {
    const unique = [...new Map(decks.map((deck) => [deck.id, deck])).values()];
    const exported: DeckExportOutcome[] = [];
    for (const deck of unique) {
      try {
        const outcome = await this.exportDeck(deck);
        if (!outcome) {
          return {
            requestedCount: unique.length,
            exported,
            cancelled: true,
          };
        }
        exported.push(outcome);
      } catch (error: unknown) {
        throw new Error(`Could not export “${deck.name}”: ${getErrorMessage(error)}`);
      }
    }
    return {
      requestedCount: unique.length,
      exported,
      cancelled: false,
    };
  }

  async shareDeck(deck: DeckSummary): Promise<DeckShareOutcome | null> {
    const prepared = await this.#decks.prepareExport(
      deck.id,
      this.#dependencies.createOperationId(),
    );
    const launch = await this.#dependencies.share(prepared.preparedExport.id);
    return launch.launched ? { deck, cardCount: prepared.cardCount, launch } : null;
  }

  invalidate(): void {
    this.#decks.invalidate();
  }
}

function ankiCompatibilitySummary(report: AnkiImportReport): string {
  const details = [`${report.skippedCardCount} Anki cards skipped`];
  if (report.simplifiedCardCount > 0) {
    details.push(`${report.simplifiedCardCount} cards required content simplification`);
  }
  if (report.typedAnswerCardCount > 0) {
    details.push(`${report.typedAnswerCardCount} typed-answer cards converted to ordinary reveal`);
  }
  if (report.nativeClozeCardCount > 0) {
    details.push(`${report.nativeClozeCardCount} cloze cards converted to native Fill in the blank`);
  }
  return ` ${details.join("; ")}.`;
}

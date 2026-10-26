import { createOperationId, getErrorCode, getErrorMessage } from "../ipc/common";
import { bulkCreateCards, type BulkCreateCardsInput, type BulkCreateResult } from "../ipc/cards";
import { createDeck, type AnkiImportReport, type CreateDeckInput, type DeckSummary } from "../ipc/decks";
import type { PublishReceipt, ShareLaunch, StagedExternalFile } from "../ipc/externalFiles";
import {
  listenToOperationProgress,
  type OperationProgress,
} from "../ipc/operationProgress";
import {
  publishExport,
  selectExternalImport,
  shareExport,
} from "../platform/files";
import { DecksController } from "./decksController";
import type { QuizletCard } from "./quizletImport";

export interface DeckImportOutcome {
  deckId: string;
  committedDeck: DeckSummary | null;
  message: string;
  ankiReport?: AnkiImportReport;
}

/** The native bulk-card limit for one write. */
const PASTED_CARDS_PER_WRITE = 500;

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
  listenProgress?(
    operationId: string,
    listener: (progress: OperationProgress) => void,
  ): Promise<() => void>;
  createDeck?(input: CreateDeckInput): Promise<DeckSummary>;
  bulkCreateCards?(input: BulkCreateCardsInput): Promise<BulkCreateResult>;
}

const defaultDependencies: DeckTransferDependencies = {
  selectImport: () => selectExternalImport("study_content"),
  publish: publishExport,
  share: shareExport,
  createDecksController: () => new DecksController(),
  createOperationId,
  listenProgress: listenToOperationProgress,
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
  /** The deck a pasted import made, kept so that pressing Import again fills it instead of making another. */
  #pastedDeck: { name: string; deck: DeckSummary } | null = null;

  constructor(dependencies: DeckTransferDependencies = defaultDependencies) {
    this.#dependencies = dependencies;
    this.#decks = dependencies.createDecksController();
  }

  async importFromFile(
    onProgress?: (progress: OperationProgress) => void,
  ): Promise<DeckImportOutcome | null> {
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
      let unlisten: (() => void) | null = null;
      if (selected.detectedKind === "anki_apkg" && onProgress) {
        try {
          unlisten = await (this.#dependencies.listenProgress ?? listenToOperationProgress)(
            pending.operationId,
            onProgress,
          );
        } catch {
          // Progress is advisory UI state. A listener transport problem must not
          // turn a valid, retry-safe import into a failed data operation.
        }
      }
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
      } finally {
        unlisten?.();
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

  /**
   * Makes a deck from pasted cards (Quizlet's Export text). The deck is made
   * once; cards are added in native-sized writes, and the native exact-duplicate
   * check makes a repeated write add only the cards still missing.
   */
  async importPastedCards(name: string, cards: readonly QuizletCard[]): Promise<DeckImportOutcome> {
    if (cards.length === 0) throw new Error("Paste at least one card with a term and a definition.");
    const deckName = name.trim();
    if (deckName === "") throw new Error("Name the new deck.");
    let deck = this.#pastedDeck?.name === deckName ? this.#pastedDeck.deck : null;
    if (!deck) {
      deck = await (this.#dependencies.createDeck ?? createDeck)({ name: deckName, description: "Imported from Quizlet." });
      this.#pastedDeck = { name: deckName, deck };
    }
    const write = this.#dependencies.bulkCreateCards ?? bulkCreateCards;
    let created = 0;
    let duplicates = 0;
    for (let start = 0; start < cards.length; start += PASTED_CARDS_PER_WRITE) {
      const rows = cards.slice(start, start + PASTED_CARDS_PER_WRITE).map((card) => ({ ...card, tags: [] }));
      try {
        const result = await write({ deckId: deck.id, rows });
        created += result.createdCount;
        duplicates += result.skippedDuplicates;
      } catch (error: unknown) {
        if (start === 0) throw error;
        throw new Error(`Added ${created} of ${cards.length} cards to “${deck.name}” before an error: ${getErrorMessage(error)} Import again to add the rest.`);
      }
    }
    this.#pastedDeck = null;
    const duplicateMessage = duplicates > 0
      ? ` ${duplicates} exact ${duplicates === 1 ? "duplicate was" : "duplicates were"} skipped.`
      : "";
    return {
      deckId: deck.id,
      committedDeck: deck,
      message: `Imported “${deck.name}” with ${created} ${created === 1 ? "card" : "cards"}.${duplicateMessage}`,
    };
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
  const typedAnswerCardCount = report.typedAnswerCardCount ?? 0;
  const nativeClozeCardCount = report.nativeClozeCardCount ?? 0;
  const javascriptSalvagedCardCount = report.javascriptSalvagedCardCount ?? 0;
  if (typedAnswerCardCount > 0) {
    details.push(`${typedAnswerCardCount} typed-answer cards converted to ordinary reveal`);
  }
  if (nativeClozeCardCount > 0) {
    details.push(`${nativeClozeCardCount} cloze cards converted to native Fill in the blank`);
  }
  if (javascriptSalvagedCardCount > 0) {
    details.push(`${javascriptSalvagedCardCount} JavaScript cards recovered as static content`);
  }
  return ` ${details.join("; ")}.`;
}

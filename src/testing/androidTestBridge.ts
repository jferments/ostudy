import { getRecentPracticeReviews } from "../ipc/analytics";
import { createCardNote } from "../ipc/cards";
import {
  createOperationId,
  createSubmissionId,
  getErrorMessage,
  type RichDocument,
} from "../ipc/common";
import {
  createDeck,
  listDecks,
  prepareDeckExport,
  type DeckSummary,
} from "../ipc/decks";
import type { ExternalImportPurpose } from "../ipc/externalFiles";
import {
  completePracticeSession,
  markPracticeCardRevealed,
  resumePracticeSession,
  startSelectedPractice,
  submitPracticeReview,
} from "../ipc/practice";
import { getUiCheckpoint, saveUiCheckpoint } from "../ipc/uiCheckpoint";
import { publishExport, selectExternalImport } from "../platform/files";

const SMOKE_DECK_NAME = "Android Emulator Smoke Deck";

interface AndroidSmokeSnapshot {
  deckCount: number;
  smokeDeckId: string | null;
  smokeCardCount: number;
  recentReviewCount: number;
}

interface AndroidPracticeRestoreSnapshot {
  sessionId: string | null;
  resumePosition: number | null;
  revealed: boolean;
  completed: boolean;
}

type AndroidFileOperationKind =
  | "image_import"
  | "pdf_document_import"
  | "study_content_import"
  | "library_backup_import"
  | "deck_export";
type AndroidFileOperationPhase = "idle" | "pending" | "settled" | "error";

interface AndroidFileOperationResult {
  displayName: string;
  detectedKind: string | null;
  byteSize: number | null;
  destinationKind: string | null;
}

interface AndroidFileOperationSnapshot {
  sequence: number;
  kind: AndroidFileOperationKind | null;
  phase: AndroidFileOperationPhase;
  cancelled: boolean | null;
  error: string | null;
  result: AndroidFileOperationResult | null;
}

interface AndroidTestBridge {
  seedAndReview(): Promise<AndroidSmokeSnapshot>;
  snapshot(): Promise<AndroidSmokeSnapshot>;
  beginInterruptedPractice(): Promise<AndroidPracticeRestoreSnapshot>;
  restorationSnapshot(): Promise<AndroidPracticeRestoreSnapshot>;
  startImportPicker(purpose: ExternalImportPurpose): AndroidFileOperationSnapshot;
  startDeckExportPicker(): AndroidFileOperationSnapshot;
  fileOperationSnapshot(): AndroidFileOperationSnapshot;
}

declare global {
  interface Window {
    __OSTUDY_ANDROID_TEST__?: AndroidTestBridge;
  }
}

let fileOperation: AndroidFileOperationSnapshot = {
  sequence: 0,
  kind: null,
  phase: "idle",
  cancelled: null,
  error: null,
  result: null,
};

/**
 * Installs a narrow E2E bridge only in APKs built explicitly for emulator
 * smoke testing. Release/package verification rejects this marker, so ordinary
 * builds cannot expose test-only mutation helpers.
 */
export function installAndroidTestBridge(): void {
  if (import.meta.env.VITE_OSTUDY_ANDROID_TEST_BRIDGE !== "1") return;
  window.__OSTUDY_ANDROID_TEST__ = {
    seedAndReview,
    snapshot,
    beginInterruptedPractice,
    restorationSnapshot,
    startImportPicker,
    startDeckExportPicker,
    fileOperationSnapshot,
  };
}

async function seedAndReview(): Promise<AndroidSmokeSnapshot> {
  const deck = await ensureSmokeDeck();
  const session = await startSelectedPractice({
    operationId: createOperationId(),
    deckIds: [deck.id],
    tagIds: [],
    shuffle: false,
    mode: "extra",
  });
  const card = session.firstWindow.cards[0];
  if (!card) throw new Error("The Android smoke deck did not produce a practice card.");
  const preview = await markPracticeCardRevealed(session.id, card.presentation.cardId);
  await submitPracticeReview({
    sessionId: session.id,
    cardId: card.presentation.cardId,
    submissionId: createSubmissionId(),
    outcome: "very_confident",
    responseTimeMs: 250,
    scheduleRevision: preview.scheduleRevision,
  });
  await completePracticeSession(session.id);
  return snapshot();
}

async function beginInterruptedPractice(): Promise<AndroidPracticeRestoreSnapshot> {
  const deck = await ensureSmokeDeck();
  const session = await startSelectedPractice({
    operationId: createOperationId(),
    deckIds: [deck.id],
    tagIds: [],
    shuffle: false,
    mode: "extra",
  });
  const card = session.firstWindow.cards[0];
  if (!card) throw new Error("The Android smoke deck did not produce a resumable card.");
  await markPracticeCardRevealed(session.id, card.presentation.cardId);
  await saveUiCheckpoint({
    section: "practice",
    practice: {
      sessionId: session.id,
      position: session.firstWindow.startPosition,
      revealed: true,
    },
  });
  return {
    sessionId: session.id,
    resumePosition: session.firstWindow.startPosition,
    revealed: true,
    completed: false,
  };
}

async function restorationSnapshot(): Promise<AndroidPracticeRestoreSnapshot> {
  const checkpoint = await getUiCheckpoint();
  if (!checkpoint?.practice) {
    return {
      sessionId: null,
      resumePosition: null,
      revealed: false,
      completed: true,
    };
  }
  const restored = await resumePracticeSession(checkpoint.practice.sessionId);
  return {
    sessionId: restored.session.id,
    resumePosition: restored.resumePosition,
    revealed: restored.revealed,
    completed: restored.completed,
  };
}

function startImportPicker(purpose: ExternalImportPurpose): AndroidFileOperationSnapshot {
  const kind = `${purpose}_import` as AndroidFileOperationKind;
  return startTrackedFileOperation(kind, async () => {
    const selected = await selectExternalImport(purpose);
    if (!selected) return null;
    return {
      displayName: selected.displayName,
      detectedKind: selected.detectedKind,
      byteSize: selected.byteSize,
      destinationKind: null,
    };
  });
}

function startDeckExportPicker(): AndroidFileOperationSnapshot {
  return startTrackedFileOperation("deck_export", async () => {
    const deck = await ensureSmokeDeck();
    const prepared = await prepareDeckExport(deck.id, createOperationId());
    const published = await publishExport(prepared.preparedExport.id);
    if (!published) return null;
    return {
      displayName: published.displayName,
      detectedKind: null,
      byteSize: prepared.preparedExport.byteSize,
      destinationKind: published.destinationKind,
    };
  });
}

function startTrackedFileOperation(
  kind: AndroidFileOperationKind,
  operation: () => Promise<AndroidFileOperationResult | null>,
): AndroidFileOperationSnapshot {
  const sequence = fileOperation.sequence + 1;
  fileOperation = {
    sequence,
    kind,
    phase: "pending",
    cancelled: null,
    error: null,
    result: null,
  };
  void Promise.resolve()
    .then(operation)
    .then((result) => {
      if (fileOperation.sequence !== sequence) return;
      fileOperation = {
        sequence,
        kind,
        phase: "settled",
        cancelled: result === null,
        error: null,
        result,
      };
    })
    .catch((error: unknown) => {
      if (fileOperation.sequence !== sequence) return;
      fileOperation = {
        sequence,
        kind,
        phase: "error",
        cancelled: null,
        error: getErrorMessage(error),
        result: null,
      };
    });
  return fileOperationSnapshot();
}

function fileOperationSnapshot(): AndroidFileOperationSnapshot {
  return {
    ...fileOperation,
    result: fileOperation.result ? { ...fileOperation.result } : null,
  };
}

async function snapshot(): Promise<AndroidSmokeSnapshot> {
  const [decks, reviews] = await Promise.all([listDecks(), getRecentPracticeReviews()]);
  const smoke = decks.find((deck) => deck.name === SMOKE_DECK_NAME) ?? null;
  return {
    deckCount: decks.length,
    smokeDeckId: smoke?.id ?? null,
    smokeCardCount: smoke?.generatedCardCount ?? 0,
    recentReviewCount: reviews.length,
  };
}

async function ensureSmokeDeck(): Promise<DeckSummary> {
  const current = await listDecks();
  let deck = current.find((candidate) => candidate.name === SMOKE_DECK_NAME);
  if (!deck) {
    deck = await createDeck({
      name: SMOKE_DECK_NAME,
      description: "Created by the official Android emulator smoke test.",
    });
  }
  if (deck.generatedCardCount === 0) {
    const deckId = deck.id;
    await createCardNote({
      cardType: "basic",
      deckId,
      frontDoc: richDocument("What platform is this smoke test exercising?"),
      backDoc: richDocument("Android"),
      tags: ["android-smoke"],
      reverseEnabled: false,
      allowDuplicate: false,
    });
    const refreshed = (await listDecks()).find((candidate) => candidate.id === deckId);
    if (!refreshed) throw new Error("The Android smoke deck disappeared after card creation.");
    deck = refreshed;
  }
  return deck;
}

function richDocument(text: string): RichDocument {
  return {
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [{ type: "text", text }],
      },
    ],
  };
}

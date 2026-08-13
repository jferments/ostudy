import { getRecentPracticeReviews } from "../ipc/analytics";
import { createCardNote } from "../ipc/cards";
import {
  createOperationId,
  createSubmissionId,
  type RichDocument,
} from "../ipc/common";
import {
  createDeck,
  listDecks,
  prepareDeckExport,
  type DeckSummary,
} from "../ipc/decks";
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

interface AndroidTestBridge {
  seedAndReview(): Promise<AndroidSmokeSnapshot>;
  snapshot(): Promise<AndroidSmokeSnapshot>;
  beginInterruptedPractice(): Promise<AndroidPracticeRestoreSnapshot>;
  restorationSnapshot(): Promise<AndroidPracticeRestoreSnapshot>;
  openImportPicker(purpose: "image" | "study_content"): void;
  openDeckExportPicker(): void;
}

declare global {
  interface Window {
    __OSTUDY_ANDROID_TEST__?: AndroidTestBridge;
  }
}

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
    openImportPicker,
    openDeckExportPicker,
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

function openImportPicker(purpose: "image" | "study_content"): void {
  void selectExternalImport(purpose).catch(() => undefined);
}

function openDeckExportPicker(): void {
  void ensureSmokeDeck()
    .then((deck) => prepareDeckExport(deck.id, createOperationId()))
    .then((prepared) => publishExport(prepared.preparedExport.id))
    .catch(() => undefined);
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
    await createCardNote({
      cardType: "basic",
      deckId: deck.id,
      frontDoc: richDocument("What platform is this smoke test exercising?"),
      backDoc: richDocument("Android"),
      tags: ["android-smoke"],
      reverseEnabled: false,
      allowDuplicate: false,
    });
    const refreshed = (await listDecks()).find((candidate) => candidate.id === deck.id);
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

import type { Analytics } from "../analytics-ipc";
import type {
  AppSettings,
  Asset,
  DeckSummary,
  Flashcard,
  OrganizedCard,
  PracticeSession,
  RichDocument,
} from "../ipc";

let nextIdentifier = 1;

export function testEntityId(): string {
  const value = nextIdentifier.toString(16).padStart(32, "0");
  nextIdentifier += 1;
  return value;
}

export function resetTestEntityIds(): void {
  nextIdentifier = 1;
}

export function paragraphDocument(text = "Content"): RichDocument {
  return {
    type: "doc",
    content:
      text === ""
        ? [{ type: "paragraph" }]
        : [
            {
              type: "paragraph",
              content: [{ type: "text", text }],
            },
          ],
  };
}

export function makeSettings(overrides: Partial<AppSettings> = {}): AppSettings {
  return {
    theme: "dark",
    uiFont: "system",
    cardFont: "system",
    uiScale: 100,
    cardBaseSize: 18,
    reduceMotion: false,
    confirmDeletions: true,
    practiceShuffle: true,
    desiredRetention: 0.9,
    dailyNewLimit: 20,
    dailyReviewLimit: 200,
    ...overrides,
  };
}

export function makeDeck(overrides: Partial<DeckSummary> = {}): DeckSummary {
  return {
    id: testEntityId(),
    name: "Test deck",
    description: "",
    sourceNoteCount: 1,
    generatedCardCount: 1,
    studyableDueCount: 1,
    archivedAt: null,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
    ...overrides,
  };
}

export function makeAsset(overrides: Partial<Asset> = {}): Asset {
  return {
    id: testEntityId(),
    originalFilename: "diagram.png",
    mimeType: "image/png",
    byteSize: 128,
    filePath: "/tmp/ostudy/diagram.png",
    createdAt: 1,
    ...overrides,
  };
}

export function makeFlashcard(overrides: Partial<Flashcard> = {}): Flashcard {
  const deckId = overrides.deckId ?? testEntityId();
  return {
    id: testEntityId(),
    noteId: testEntityId(),
    deckId,
    direction: "forward",
    frontDoc: paragraphDocument("Question"),
    backDoc: paragraphDocument("Answer"),
    frontText: "Question",
    backText: "Answer",
    assets: [],
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
    ...overrides,
  };
}

export function makeOrganizedCard(
  overrides: Partial<OrganizedCard> = {},
): OrganizedCard {
  return {
    ...makeFlashcard(overrides),
    tags: [],
    dueAt: Date.now(),
    ...overrides,
  };
}

export function makePracticeSession(
  overrides: Partial<PracticeSession> = {},
): PracticeSession {
  const deck = makeDeck();
  const card = makeFlashcard({ deckId: deck.id });
  return {
    id: testEntityId(),
    cardCount: 1,
    firstWindow: {
      startPosition: 0,
      cards: [
        {
          card,
          deckName: deck.name,
          dueAt: Date.now(),
          scheduleState: 0,
          needsMorePractice: false,
        },
      ],
      hasMore: false,
    },
    shuffle: false,
    mode: "scheduled",
    startedAt: Date.now(),
    ...overrides,
  };
}

export function makeAnalytics(overrides: Partial<Analytics> = {}): Analytics {
  const base: Analytics = {
    scope: {
      range: "7d",
      startLocalDate: "2026-08-01",
      endLocalDate: "2026-08-06",
      startAt: 1,
      endAt: 2,
      bucketGranularity: "day",
      deckIds: [],
      tagIds: [],
      highlightedSessionId: null,
    },
    currentInventory: {
      currentDeckCount: 0,
      sourceNoteCount: 0,
      generatedCardCount: 0,
      studyableCardCount: 0,
      studyableDueCount: 0,
      needsMorePracticeCount: 0,
    },
    history: {
      cardsPracticed: 0,
      reviewCount: 0,
      correctCount: 0,
      correctRate: null,
      averageConfidence: null,
      averageResponseTimeMs: null,
      totalStudyTimeMs: 0,
      sessionCount: 0,
    },
    dailyLimits: {
      newLimit: 20,
      reviewLimit: 200,
      newCompleted: 0,
      reviewsCompleted: 0,
      newRemaining: 20,
      reviewsRemaining: 200,
    },
    ratingDistribution: { again: 0, hard: 0, good: 0, easy: 0 },
    timeline: [],
    deckStats: [],
    tagStats: [],
    recentReviews: [],
    recentSessions: [],
    attributionCoverage: {
      capturedReviewCount: 0,
      legacyBackfilledReviewCount: 0,
    },
    dimensionOptions: {
      decks: [],
      tags: [],
    },
  };
  return { ...base, ...overrides };
}

export interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T | PromiseLike<T>) => void;
  reject: (reason?: unknown) => void;
}

export function deferred<T>(): Deferred<T> {
  let resolve!: Deferred<T>["resolve"];
  let reject!: Deferred<T>["reject"];
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

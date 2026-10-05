import type { Analytics } from "../ipc/analytics";
import {
  type Asset,
} from "../ipc/assets";
import {
  type CardPresentation,
  type CardNote,
} from "../ipc/cards";
import {
  type RichDocument,
} from "../ipc/common";
import {
  type DeckSummary,
} from "../ipc/decks";
import type { HomeSubjectSummary, HomeSummary, HomeToday } from "../ipc/home";
import {
  type PracticeSession,
} from "../ipc/practice";
import {
  type AppSettings,
} from "../ipc/settings";
import type { Subject, SubjectLibrary } from "../ipc/subjects";

let nextIdentifier = 1;

export function testEntityId(): string {
  const value = nextIdentifier.toString(16).padStart(32, "0");
  nextIdentifier += 1;
  return value;
}

export function resetTestEntityIds(): void {
  nextIdentifier = 1;
}

export function requiredTestValue<T>(
  value: T | null | undefined,
  label = "test fixture value",
): T {
  if (value === null || value === undefined) {
    throw new Error(`Missing required ${label}.`);
  }
  return value;
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
    motionPreference: "system",
    materialPreference: "automatic",
    decorativeArtwork: true,
    celebrationsEnabled: true,
    tutorialsEnabled: false,
    confirmDeletions: true,
    practiceShuffle: true,
    displayQuestionOnAnswer: false,
    desiredRetention: 0.9,
    dailyNewLimit: 20,
    dailyReviewLimit: 200,
    newDayStartHour: 4,
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
    createdAt: 1,
    ...overrides,
  };
}

export function makeCardPresentation(
  overrides: Partial<CardPresentation> = {},
): CardPresentation {
  const deckId = overrides.deckId ?? testEntityId();
  return {
    cardId: testEntityId(),
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
    noteRevision: 1,
    ...overrides,
  };
}

export function makeCardNote(
  overrides: Partial<Extract<CardNote, { cardType: "basic" }>> = {},
): CardNote {
  return {
    cardType: "basic",
    noteId: testEntityId(),
    deckId: testEntityId(),
    frontDoc: paragraphDocument("Question"),
    backDoc: paragraphDocument("Answer"),
    frontText: "Question",
    backText: "Answer",
    reverseEnabled: false,
    assets: [],
    tags: [],
    generatedCardCount: 1,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
    ...overrides,
  };
}

export function makePracticeSession(
  overrides: Partial<PracticeSession> = {},
): PracticeSession {
  const deck = makeDeck();
  const card = makeCardPresentation({ deckId: deck.id });
  return {
    id: testEntityId(),
    cardCount: 1,
    firstWindow: {
      startPosition: 0,
      cards: [
        {
          presentation: card,
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
      todayLocalDate: "2026-08-06",
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
      readyReviewCount: 0,
      readyNewCount: 0,
      dueTodayReviewCount: 0,
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
      retentionReviewCount: 0,
      retentionRememberedCount: 0,
      trueRetention: null,
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
    questionTypes: [],
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

export function makeSubject(overrides: Partial<Subject> = {}): Subject {
  return {
    id: testEntityId(),
    name: "Test subject",
    sortOrder: 0,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
    ...overrides,
  };
}

export function makeSubjectLibrary(overrides: Partial<SubjectLibrary> = {}): SubjectLibrary {
  return {
    libraryRevision: 1,
    documentFilingRevision: 1,
    subjects: [],
    decks: [],
    documents: [],
    studyRanges: [],
    checkTheseCount: 0,
    ...overrides,
  };
}

export function makeHomeSubjectSummary(
  overrides: Partial<HomeSubjectSummary> = {},
): HomeSubjectSummary {
  return {
    subjectId: null,
    dueReviews: 0,
    dueNew: 0,
    scheduledReviews: 0,
    scheduledNew: 0,
    cardCount: 0,
    deckCount: 0,
    sourceCount: 0,
    hasOverviewGuide: false,
    dueDeckIds: [],
    ...overrides,
  };
}

export function makeHomeSummary(
  overrides: Partial<Omit<HomeSummary, "today">> & { today?: Partial<HomeToday> } = {},
): HomeSummary {
  const { today, ...rest } = overrides;
  return {
    studyDate: "2026-10-05",
    subjects: [],
    noSubject: makeHomeSubjectSummary(),
    ...rest,
    today: {
      dueReviews: 0,
      dueNew: 0,
      scheduledReviews: 0,
      scheduledNew: 0,
      reviewsRemaining: 200,
      newRemaining: 20,
      subjectCount: 0,
      deckIds: [],
      estimatedMinutes: 0,
      estimateMsPerCard: 12_000,
      estimateReviewCount: 0,
      ...today,
    },
  };
}

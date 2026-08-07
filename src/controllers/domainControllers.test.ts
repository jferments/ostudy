import { beforeEach, describe, expect, it, vi } from "vitest";

import { deferred } from "../test/factories";
import * as analyticsIpc from "../ipc/analytics";
import * as cardsIpc from "../ipc/cards";
import * as decksIpc from "../ipc/decks";
import * as practiceIpc from "../ipc/practice";
import * as templatesIpc from "../ipc/templates";
import { AnalyticsController } from "./analyticsController";
import { CardEditorController } from "./cardEditorController";
import { CardsController } from "./cardsController";
import { DecksController } from "./decksController";
import { PracticeController } from "./practiceController";

vi.mock("../ipc/analytics", () => ({
  getAnalytics: vi.fn(),
  getRecentPracticeReviews: vi.fn(),
}));
vi.mock("../ipc/cards", () => ({
  getCardNote: vi.fn(),
  listMatchingCards: vi.fn(),
  listTags: vi.fn(),
  searchCards: vi.fn(),
}));
vi.mock("../ipc/decks", () => ({
  getDeck: vi.fn(),
  listDeckCardSummaries: vi.fn(),
  listDecks: vi.fn(),
}));
vi.mock("../ipc/practice", () => ({
  getPracticeCardWindow: vi.fn(),
  getPracticeWorkload: vi.fn(),
}));
vi.mock("../ipc/templates", () => ({
  getCardTemplate: vi.fn(),
  listCardTemplateSummaries: vi.fn(),
  previewTemplateCards: vi.fn(),
}));

const searchInput: cardsIpc.CardSearchInput = {
  deckIds: [],
  query: "",
  tagIds: [],
  availabilityFilter: "all",
  statusFilter: "active",
  reviewedFilter: "all",
  sort: "updated_desc",
  limit: 100,
  offset: 0,
};

beforeEach(() => {
  vi.clearAllMocks();
});

describe("domain controllers ignore obsolete read completions", () => {
  it("keeps the newest Search/Filter page when requests resolve backwards", async () => {
    const controller = new CardsController();
    const older = deferred<cardsIpc.CardSearchPage>();
    const newer = deferred<cardsIpc.CardSearchPage>();
    vi.mocked(cardsIpc.searchCards)
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise);

    const oldResult = controller.search({ ...searchInput, query: "ACE" });
    const newResult = controller.search({ ...searchInput, query: "beta" });
    const expected = { cards: [], totalCount: 7 };
    newer.resolve(expected);
    expect(await newResult).toEqual(expected);
    older.resolve({ cards: [], totalCount: 99 });
    expect(await oldResult).toBeUndefined();
  });

  it("keeps the newest analytics scope when requests resolve backwards", async () => {
    const controller = new AnalyticsController();
    const older = deferred<analyticsIpc.Analytics>();
    const newer = deferred<analyticsIpc.Analytics>();
    vi.mocked(analyticsIpc.getAnalytics)
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise);

    const oldResult = controller.load({ range: "7d", deckIds: [], tagIds: [] });
    const newResult = controller.load({ range: "90d", deckIds: [], tagIds: [] });
    const expected = { scope: { range: "90d" } } as analyticsIpc.Analytics;
    newer.resolve(expected);
    expect(await newResult).toBe(expected);
    older.resolve({ scope: { range: "7d" } } as analyticsIpc.Analytics);
    expect(await oldResult).toBeUndefined();
  });

  it("prevents an older template load from replacing the newly selected template", async () => {
    const controller = new CardEditorController();
    const older = deferred<templatesIpc.CardTemplate>();
    const newer = deferred<templatesIpc.CardTemplate>();
    vi.mocked(templatesIpc.getCardTemplate)
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise);

    const oldResult = controller.template("a".repeat(32));
    const newResult = controller.template("b".repeat(32));
    const expected = { id: "b".repeat(32), name: "new" } as templatesIpc.CardTemplate;
    newer.resolve(expected);
    expect(await newResult).toBe(expected);
    older.resolve({ id: "a".repeat(32), name: "old" } as templatesIpc.CardTemplate);
    expect(await oldResult).toBeUndefined();
  });

  it("prevents an older Practice workload from replacing a newer scope", async () => {
    const controller = new PracticeController();
    const older = deferred<practiceIpc.PracticeWorkloadOverview>();
    const newer = deferred<practiceIpc.PracticeWorkloadOverview>();
    vi.mocked(practiceIpc.getPracticeWorkload)
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise);

    const oldResult = controller.workload(["a".repeat(32)], []);
    const newResult = controller.workload(["b".repeat(32)], []);
    const expected = { studyableDueReviews: 2 } as practiceIpc.PracticeWorkloadOverview;
    newer.resolve(expected);
    expect(await newResult).toBe(expected);
    older.resolve({ studyableDueReviews: 90 } as practiceIpc.PracticeWorkloadOverview);
    expect(await oldResult).toBeUndefined();
  });

  it("prevents an older deck-detail page from replacing a newer query", async () => {
    const controller = new DecksController();
    const older = deferred<decksIpc.DeckCardSummaryPage>();
    const newer = deferred<decksIpc.DeckCardSummaryPage>();
    vi.mocked(decksIpc.listDeckCardSummaries)
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise);
    const base = {
      deckId: "d".repeat(32),
      sort: "updated_desc" as const,
      limit: 50,
      offset: 0,
    };

    const oldResult = controller.cards({ ...base, query: "ACE" });
    const newResult = controller.cards({ ...base, query: "beta" });
    const expected = { cards: [], totalCount: 1, offset: 0, hasMore: false };
    newer.resolve(expected);
    expect(await newResult).toEqual(expected);
    older.resolve({ cards: [], totalCount: 50, offset: 0, hasMore: true });
    expect(await oldResult).toBeUndefined();
  });
});

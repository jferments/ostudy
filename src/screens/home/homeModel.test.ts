import { describe, expect, it } from "vitest";

import {
  makeHomeSubjectSummary,
  makeHomeSummary,
  makeSubject,
  makeSubjectLibrary,
} from "../../test/factories";
import {
  buildSubjectTiles,
  countLabel,
  formatStudyDate,
  sortSubjectTiles,
  subjectColorIndexes,
  subjectInitials,
  type SubjectTile,
} from "./homeModel";

describe("subjectInitials", () => {
  it.each([
    ["Cardiac physiology", "CP"],
    ["Pharmacology: Autonomic", "PA"],
    ["Spanish for clinicians", "SC"],
    ["Organic chemistry II", "OC"],
    ["Biology", "BI"],
    ["the Art of War", "AW"],
    ["Of", "OF"],
    ["  émile   zola ", "ÉZ"],
    ["日本語 文法", "日文"],
    ["2nd year anatomy", "2Y"],
    ["(Draft) notes", "DN"],
    ["!!!", "?"],
  ])("gives %s the initials %s", (name, initials) => {
    expect(subjectInitials(name)).toBe(initials);
  });
});

describe("subjectColorIndexes", () => {
  it("colors subjects by creation order, wrapping after six, whatever their order", () => {
    const subjects = Array.from({ length: 8 }, (_, index) =>
      makeSubject({ id: `s${index}`, createdAt: 100 - index, sortOrder: index }),
    );
    const colors = subjectColorIndexes(subjects);
    // The last listed was created first.
    expect(colors.get("s7")).toBe(0);
    expect(colors.get("s2")).toBe(5);
    expect(colors.get("s1")).toBe(0);
    expect(colors.get("s0")).toBe(1);
  });

  it("breaks creation-time ties by id so the colors are stable", () => {
    const colors = subjectColorIndexes([
      makeSubject({ id: "b", createdAt: 1 }),
      makeSubject({ id: "a", createdAt: 1 }),
    ]);
    expect(colors.get("a")).toBe(0);
    expect(colors.get("b")).toBe(1);
  });
});

describe("formatStudyDate", () => {
  it("names the study date without shifting it by time zone", () => {
    expect(formatStudyDate("2026-10-05")).toBe("Monday, October 5");
    expect(formatStudyDate("2026-01-01")).toBe("Thursday, January 1");
    expect(formatStudyDate("not a date")).toBe("not a date");
  });
});

describe("countLabel", () => {
  it("chooses the singular for one and groups large counts", () => {
    expect(countLabel(1, "card")).toBe("1 card");
    expect(countLabel(0, "card")).toBe("0 cards");
    expect(countLabel(1234, "source")).toBe("1,234 sources");
  });
});

describe("buildSubjectTiles", () => {
  const cardiac = makeSubject({ id: "cardiac", name: "Cardiac physiology", createdAt: 1 });
  const spanish = makeSubject({ id: "spanish", name: "Spanish for clinicians", createdAt: 2 });

  it("keeps the learner's order and shows a subject the summary does not know yet as empty", () => {
    const summary = makeHomeSummary({
      subjects: [
        makeHomeSubjectSummary({ subjectId: "spanish", scheduledReviews: 9, cardCount: 312 }),
      ],
    });
    const library = makeSubjectLibrary({ subjects: [spanish, cardiac] });
    const { subjects, noSubject } = buildSubjectTiles(summary, library, new Map());

    expect(subjects.map((tile) => [tile.name, tile.initials, tile.position, tile.dueCount])).toEqual([
      ["Spanish for clinicians", "SC", 0, 9],
      ["Cardiac physiology", "CP", 1, 0],
    ]);
    expect(subjects[1]?.summary.cardCount).toBe(0);
    expect(subjects.map((tile) => tile.colorIndex)).toEqual([1, 0]);
    expect(noSubject).toBeNull();
  });

  it("adds No subject only when unfiled decks or documents exist", () => {
    const library = makeSubjectLibrary({ subjects: [cardiac] });
    const withDecks = buildSubjectTiles(
      makeHomeSummary({ noSubject: makeHomeSubjectSummary({ deckCount: 2, scheduledNew: 3 }) }),
      library,
      new Map(),
    );
    expect(withDecks.noSubject).toMatchObject({
      key: "no-subject",
      subjectId: null,
      name: "No subject",
      colorIndex: null,
      dueCount: 3,
      position: 1,
    });
    const withDocuments = buildSubjectTiles(
      makeHomeSummary({ noSubject: makeHomeSubjectSummary({ sourceCount: 1 }) }),
      library,
      new Map(),
    );
    expect(withDocuments.noSubject).not.toBeNull();
    const empty = buildSubjectTiles(makeHomeSummary(), library, new Map());
    expect(empty.noSubject).toBeNull();
  });

  it("marks a subject preparing while one of its documents is processed", () => {
    const library = makeSubjectLibrary({
      subjects: [cardiac, spanish],
      documents: [
        { documentId: "doc-1", subjectId: "cardiac" },
        { documentId: "doc-2", subjectId: null },
      ],
    });
    const summary = makeHomeSummary({ noSubject: makeHomeSubjectSummary({ sourceCount: 1 }) });
    const preparations = new Map([
      ["doc-1", { step: "Building outline", documentTitle: "Week 4 notes" }],
      ["doc-2", { step: "Reading document structure", documentTitle: "Loose PDF" }],
    ]);
    const { subjects, noSubject } = buildSubjectTiles(summary, library, preparations);
    expect(subjects[0]?.preparing).toEqual({ step: "Building outline", documentTitle: "Week 4 notes" });
    expect(subjects[1]?.preparing).toBeNull();
    expect(noSubject?.preparing?.documentTitle).toBe("Loose PDF");
  });
});

describe("sortSubjectTiles", () => {
  function tile(name: string, position: number, dueCount: number): SubjectTile {
    return {
      key: name,
      subjectId: name,
      name,
      initials: "",
      colorIndex: 0,
      position,
      summary: makeHomeSubjectSummary({ subjectId: name }),
      dueCount,
      preparing: null,
    };
  }
  const tiles = [tile("biology 10", 0, 4), tile("Anatomy", 1, 23), tile("biology 9", 2, 4)];

  it("puts the most due first and keeps the learner's order among equals", () => {
    expect(sortSubjectTiles(tiles, "most-due").map((entry) => entry.name)).toEqual([
      "Anatomy",
      "biology 10",
      "biology 9",
    ]);
  });

  it("sorts names naturally, ignoring case", () => {
    expect(sortSubjectTiles(tiles, "name").map((entry) => entry.name)).toEqual([
      "Anatomy",
      "biology 9",
      "biology 10",
    ]);
  });

  it("returns the learner's own order", () => {
    expect(sortSubjectTiles([...tiles].reverse(), "your-order").map((entry) => entry.name)).toEqual([
      "biology 10",
      "Anatomy",
      "biology 9",
    ]);
  });
});

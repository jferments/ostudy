import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { HomeSummaryProvider } from "../../controllers/HomeSummaryProvider";
import type { DocumentSummary } from "../../ipc/documents";
import * as homeIpc from "../../ipc/home";
import * as subjectsIpc from "../../ipc/subjects";
import {
  makeDeck,
  makeHomeSubjectSummary,
  makeHomeSummary,
  makeSubject,
  makeSubjectLibrary,
} from "../../test/factories";
import { SubjectScreen } from "./SubjectScreen";

const jobs = vi.hoisted(() => ({ value: null as unknown }));

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/home", () => ({ getHomeSummary: vi.fn() }));
vi.mock("../../ipc/subjects", () => ({ getSubjectLibrary: vi.fn() }));
vi.mock("../../controllers/DocumentJobsProvider", () => ({ useDocumentJobs: () => jobs.value }));
vi.mock("../CollectionStudyGuideDialog", () => ({
  CollectionStudyGuideDialog: ({
    collectionId,
    collectionTitle,
    onClose,
  }: {
    collectionId: string;
    collectionTitle: string;
    onClose: () => void;
  }) => (
    <section>
      <h1>
        Overview guide mock {collectionId} {collectionTitle}
      </h1>
      <button type="button" onClick={onClose}>
        Close guide mock
      </button>
    </section>
  ),
}));

function makeDocument(overrides: Partial<DocumentSummary>): DocumentSummary {
  return {
    id: "doc",
    title: "Document",
    sourceFormat: "pdf",
    sourceUnitKind: "page",
    sourceUnitCount: 14,
    lifecycleState: "ready",
    generatedCardCount: 0,
    generatedDeckId: null,
    ...overrides,
  } as DocumentSummary;
}

const DECKS = [
  makeDeck({ id: "deck-guyton", name: "Guyton cards", generatedCardCount: 52 }),
  makeDeck({ id: "deck-lecture", name: "Lecture 7 cards", generatedCardCount: 41 }),
  makeDeck({ id: "deck-other", name: "Pharmacology cards" }),
  makeDeck({ id: "deck-old", name: "Archived cards", archivedAt: 5 }),
];

function renderSubject(subjectId = "cardiac") {
  const props = {
    onBack: vi.fn(),
    onStartReview: vi.fn(),
    onPracticeSubject: vi.fn(),
    onOpenLibrary: vi.fn(),
    onOpenDocuments: vi.fn(),
    showToast: vi.fn(),
  };
  render(
    <HomeSummaryProvider section="home" activeWorkflow={null} decks={DECKS}>
      <SubjectScreen subjectId={subjectId} decks={DECKS} {...props} />
    </HomeSummaryProvider>,
  );
  return props;
}

describe("SubjectScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    jobs.value = {
      isLoading: false,
      documents: [
        makeDocument({ id: "doc-guyton", title: "Guyton & Hall, Ch. 9", generatedCardCount: 52 }),
        makeDocument({
          id: "doc-notes",
          title: "Week 4 notes",
          sourceFormat: "docx",
          sourceUnitCount: 1,
          lifecycleState: "failed",
        }),
        makeDocument({ id: "doc-pharm", title: "Pharmacology notes" }),
      ],
      cancelingDocumentIds: new Set(),
      guideGeneratingDocumentIds: new Set(["doc-guyton"]),
      generationProgress: {},
    };
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(
      makeSubjectLibrary({
        subjects: [
          makeSubject({ id: "cardiac", name: "Cardiac physiology", createdAt: 1 }),
          makeSubject({ id: "pharm", name: "Pharmacology", createdAt: 2 }),
        ],
        decks: [
          { deckId: "deck-lecture", subjectId: "cardiac", sortOrder: 2 },
          { deckId: "deck-guyton", subjectId: "cardiac", sortOrder: 1 },
          { deckId: "deck-old", subjectId: "cardiac", sortOrder: 3 },
          { deckId: "deck-other", subjectId: "pharm", sortOrder: 1 },
        ],
        documents: [
          { documentId: "doc-guyton", subjectId: "cardiac" },
          { documentId: "doc-notes", subjectId: "cardiac" },
          { documentId: "doc-pharm", subjectId: "pharm" },
        ],
      }),
    );
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(
      makeHomeSummary({
        studyDate: "2026-10-05",
        subjects: [
          makeHomeSubjectSummary({
            subjectId: "cardiac",
            scheduledReviews: 20,
            scheduledNew: 3,
            dueNew: 14,
            cardCount: 111,
            deckCount: 2,
            sourceCount: 2,
            hasOverviewGuide: true,
            dueDeckIds: ["deck-guyton", "deck-lecture"],
          }),
          makeHomeSubjectSummary({ subjectId: "pharm", cardCount: 10, deckCount: 1, sourceCount: 1 }),
        ],
      }),
    );
  });

  it("shows the subject with its due count, Start and Manage", async () => {
    const props = renderSubject();
    expect(await screen.findByRole("heading", { level: 1, name: "Cardiac physiology" })).toBeTruthy();
    expect(screen.getByText("2 sources · 111 cards")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: /Review 23 due/u }));
    expect(props.onStartReview).toHaveBeenCalledWith(["deck-guyton", "deck-lecture"]);
    fireEvent.click(screen.getByRole("button", { name: "Manage" }));
    expect(props.onOpenLibrary).toHaveBeenCalledWith({ subjectId: "cardiac" });
    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    expect(props.onBack).toHaveBeenCalledOnce();
  });

  it("lists only this subject's sources with their state", async () => {
    const props = renderSubject();
    const sources = await screen.findByTestId("subject-sources");
    const rows = within(sources).getAllByTestId("subject-source");
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringContaining("Guyton & Hall, Ch. 9"),
      expect.stringContaining("Week 4 notes"),
    ]);
    expect(within(rows[0]!).getByText("14 pages · PDF")).toBeTruthy();
    expect(within(rows[0]!).getByText("Writing a study guide")).toBeTruthy();
    expect(within(rows[0]!).getByText("52 cards")).toBeTruthy();
    expect(within(rows[1]!).getByText("1 page · Word")).toBeTruthy();
    expect(within(rows[1]!).getByText("Needs attention")).toBeTruthy();
    fireEvent.click(within(rows[1]!).getByRole("button"));
    expect(props.onOpenDocuments).toHaveBeenCalledOnce();
  });

  it("lists the subject's live decks in its order and opens them in Library", async () => {
    const props = renderSubject();
    const cards = await screen.findByTestId("subject-cards");
    const decks = within(cards).getAllByTestId("subject-deck");
    expect(decks.map((deck) => deck.textContent)).toEqual(["Guyton cards52", "Lecture 7 cards41"]);
    fireEvent.click(decks[1]!);
    expect(props.onOpenLibrary).toHaveBeenCalledWith({ deckId: "deck-lecture" });
    expect(within(cards).getByText("Due today").nextElementSibling?.textContent).toBe("23");
    expect(within(cards).getByText("New").nextElementSibling?.textContent).toBe("14");

    fireEvent.click(within(cards).getByRole("button", { name: "Browse cards" }));
    expect(props.onOpenLibrary).toHaveBeenLastCalledWith({ subjectId: "cardiac" });
    fireEvent.click(within(cards).getByRole("button", { name: "Practice a mix" }));
    expect(props.onPracticeSubject).toHaveBeenCalledWith("cardiac");
  });

  it("reads the overview guide in place and returns to the subject", async () => {
    renderSubject();
    fireEvent.click(await screen.findByRole("button", { name: "Read overview" }));
    expect(
      screen.getByRole("heading", { name: "Overview guide mock cardiac Cardiac physiology" }),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Close guide mock" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Cardiac physiology" })).toBeTruthy();
  });

  it("points to Documents when there is no overview guide and hides Start when nothing is due", async () => {
    const props = renderSubject("pharm");
    expect(await screen.findByRole("heading", { level: 1, name: "Pharmacology" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /due/u })).toBeNull();
    expect(screen.queryByRole("button", { name: "Read overview" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open Documents" }));
    expect(props.onOpenDocuments).toHaveBeenCalledOnce();
  });

  it("says when the subject no longer exists", async () => {
    const props = renderSubject("gone");
    expect(await screen.findByText(/This subject no longer exists/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Back to Home" }));
    expect(props.onBack).toHaveBeenCalledOnce();
  });
});

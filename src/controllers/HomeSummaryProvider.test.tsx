import { act, render, screen } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as homeIpc from "../ipc/home";
import type { HomeSummary } from "../ipc/home";
import * as subjectsIpc from "../ipc/subjects";
import type { SubjectLibrary } from "../ipc/subjects";
import {
  deferred,
  makeDeck,
  makeHomeSummary,
  makeSubject,
  makeSubjectLibrary,
} from "../test/factories";
import { cardsDueToday, HomeSummaryProvider, useHomeSummary } from "./HomeSummaryProvider";
import type { AppSection, AppWorkflow } from "./useAppController";

vi.mock("../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../ipc/home", () => ({ getHomeSummary: vi.fn() }));
vi.mock("../ipc/subjects", () => ({ getSubjectLibrary: vi.fn() }));

const documents = vi.hoisted(() => ({ current: [] as { id: string; lifecycleState: string }[] }));
vi.mock("./DocumentJobsProvider", () => ({
  useDocumentJobs: () => ({ documents: documents.current }),
}));

function Probe() {
  const state = useHomeSummary();
  return (
    <div>
      <output aria-label="due">{cardsDueToday(state.summary) ?? "unknown"}</output>
      <output aria-label="subjects">
        {state.library?.subjects.map((subject) => subject.name).join(",") ?? "unknown"}
      </output>
      <output aria-label="loading">{state.isLoading ? "yes" : "no"}</output>
      <output aria-label="errors">{`${state.summaryError ?? ""}|${state.libraryError ?? ""}`}</output>
      <button type="button" onClick={state.refresh}>
        Refresh
      </button>
      <button
        type="button"
        onClick={() =>
          state.acceptLibrary(makeSubjectLibrary({ subjects: [makeSubject({ name: "Accepted" })] }))
        }
      >
        Accept
      </button>
    </div>
  );
}

const decks = [makeDeck()];

function renderProvider(section: AppSection = "home", activeWorkflow: AppWorkflow | null = null) {
  const view = render(
    <HomeSummaryProvider section={section} activeWorkflow={activeWorkflow} decks={decks}>
      <Probe />
    </HomeSummaryProvider>,
  );
  return {
    ...view,
    rerenderWith(
      nextSection: AppSection,
      nextWorkflow: AppWorkflow | null,
      nextDecks = decks,
      finishedSessions = 0,
      studyRules?: { dailyNewLimit: number; dailyReviewLimit: number; newDayStartHour: number },
    ) {
      view.rerender(
        <HomeSummaryProvider
          section={nextSection}
          activeWorkflow={nextWorkflow}
          decks={nextDecks}
          finishedSessions={finishedSessions}
          studyRules={studyRules}
        >
          <Probe />
        </HomeSummaryProvider>,
      );
    },
  };
}

function summaryWith(due: number): HomeSummary {
  return makeHomeSummary({ today: { scheduledReviews: due } });
}

async function settle(): Promise<void> {
  await act(async () => {
    vi.advanceTimersByTime(60);
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("HomeSummaryProvider", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    documents.current = [];
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(summaryWith(5));
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(
      makeSubjectLibrary({ subjects: [makeSubject({ name: "Cardiac" })] }),
    );
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("reads today's summary and the library once for the triggers one change sets off", async () => {
    renderProvider();
    expect(screen.getByLabelText("loading").textContent).toBe("yes");
    await settle();
    expect(screen.getByLabelText("due").textContent).toBe("5");
    expect(screen.getByLabelText("subjects").textContent).toBe("Cardiac");
    expect(screen.getByLabelText("loading").textContent).toBe("no");
    // Home being visible and the deck list arriving coalesce into one read.
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(1);
    expect(subjectsIpc.getSubjectLibrary).toHaveBeenCalledTimes(1);
  });

  it("still reads when React mounts the provider twice, as development builds do", async () => {
    render(
      <StrictMode>
        <HomeSummaryProvider section="home" activeWorkflow={null} decks={decks}>
          <Probe />
        </HomeSummaryProvider>
      </StrictMode>,
    );
    await settle();
    expect(screen.getByLabelText("loading").textContent).toBe("no");
    expect(screen.getByLabelText("due").textContent).toBe("5");
  });

  it("reads again when Home or Review opens, a session ends or the deck list changes", async () => {
    const view = renderProvider("decks");
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(1);

    view.rerenderWith("review", null);
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(2);

    view.rerenderWith("review", "practice");
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(2);
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(summaryWith(3));
    view.rerenderWith("review", null);
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(3);
    expect(screen.getByLabelText("due").textContent).toBe("3");

    view.rerenderWith("review", null, [makeDeck()]);
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(4);
  });

  it("reads again when a session reaches its summary, not for each rating", async () => {
    const view = renderProvider("review", "practice");
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(1);

    // Ratings change nothing the provider watches, so the session keeps its count.
    view.rerenderWith("review", "practice");
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(1);

    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(summaryWith(0));
    view.rerenderWith("review", "practice", decks, 1);
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(2);
    expect(screen.getByLabelText("due").textContent).toBe("0");
  });

  it("reads again when a document arrives or finishes, not while it makes progress", async () => {
    const view = renderProvider("tutor");
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(1);

    documents.current = [{ id: "doc", lifecycleState: "processing" }];
    view.rerenderWith("tutor", null);
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(2);

    documents.current = [{ id: "doc", lifecycleState: "processing" }];
    view.rerenderWith("tutor", null);
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(2);

    documents.current = [{ id: "doc", lifecycleState: "ready" }];
    view.rerenderWith("tutor", null);
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(3);
  });

  it("reads again when the saved daily limits or new-day hour change", async () => {
    const rules = { dailyNewLimit: 20, dailyReviewLimit: 200, newDayStartHour: 4 };
    const view = renderProvider("settings");
    view.rerenderWith("settings", null, decks, 0, rules);
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(1);

    // Saving another setting hands over equal rules: nothing new to count.
    view.rerenderWith("settings", null, decks, 0, { ...rules });
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(1);

    view.rerenderWith("settings", null, decks, 0, { ...rules, newDayStartHour: 0 });
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(2);

    // The Review badge on Settings follows a lower review limit.
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(summaryWith(2));
    view.rerenderWith("settings", null, decks, 0, { ...rules, dailyReviewLimit: 2 });
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(3);
    expect(screen.getByLabelText("due").textContent).toBe("2");
  });

  it("reads again when the app becomes visible, since the study day may have turned over", async () => {
    renderProvider("tutor");
    await settle();
    document.dispatchEvent(new Event("visibilitychange"));
    await settle();
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(2);
  });

  it("keeps the newest answer when an older read finishes late", async () => {
    const slow = deferred<HomeSummary>();
    vi.mocked(homeIpc.getHomeSummary)
      .mockReturnValueOnce(slow.promise)
      .mockResolvedValueOnce(summaryWith(8));
    renderProvider();
    await settle();
    act(() => screen.getByRole("button", { name: "Refresh" }).click());
    await settle();
    expect(screen.getByLabelText("due").textContent).toBe("8");
    await act(async () => {
      slow.resolve(summaryWith(1));
      await Promise.resolve();
    });
    expect(screen.getByLabelText("due").textContent).toBe("8");
  });

  it("adopts a mutation's library over a read still in flight and rereads what is due", async () => {
    const slowLibrary = deferred<SubjectLibrary>();
    vi.mocked(subjectsIpc.getSubjectLibrary).mockReturnValueOnce(slowLibrary.promise);
    renderProvider();
    await settle();
    act(() => screen.getByRole("button", { name: "Accept" }).click());
    await settle();
    expect(screen.getByLabelText("subjects").textContent).toBe("Accepted");
    expect(homeIpc.getHomeSummary).toHaveBeenCalledTimes(2);
    await act(async () => {
      slowLibrary.resolve(makeSubjectLibrary({ subjects: [makeSubject({ name: "Stale" })] }));
      await Promise.resolve();
    });
    expect(screen.getByLabelText("subjects").textContent).toBe("Accepted");
  });

  it("reports each read's failure separately and keeps the other's answer", async () => {
    vi.mocked(homeIpc.getHomeSummary).mockRejectedValue(new Error("Summary failed"));
    renderProvider();
    await settle();
    expect(screen.getByLabelText("errors").textContent).toBe("Summary failed|");
    expect(screen.getByLabelText("subjects").textContent).toBe("Cardiac");
    expect(screen.getByLabelText("due").textContent).toBe("unknown");
    expect(screen.getByLabelText("loading").textContent).toBe("no");
  });

  it("counts what Start review studies: the scheduled reviews and new cards", () => {
    expect(cardsDueToday(null)).toBeNull();
    expect(
      cardsDueToday(makeHomeSummary({ today: { scheduledReviews: 4, scheduledNew: 3, dueReviews: 40 } })),
    ).toBe(7);
  });
});

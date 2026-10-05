import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as decksIpc from "../ipc/decks";
import type { LifecycleSnapshot } from "../ipc/platform";
import * as settingsIpc from "../ipc/settings";
import * as checkpointIpc from "../ipc/uiCheckpoint";
import { dispatchPlatformBack } from "../platform/backNavigation";
import { makeDeck, makeSettings } from "../test/factories";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { useAppController } from "./useAppController";

let lifecycleListener: ((snapshot: LifecycleSnapshot) => void) | null = null;

vi.mock("../ipc/common", () => ({
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));
vi.mock("../ipc/decks", () => ({ listDecks: vi.fn() }));
vi.mock("../ipc/settings", () => ({ getSettings: vi.fn() }));
vi.mock("../ipc/platform", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../ipc/platform")>();
  return {
    ...actual,
    listenToLifecycle: vi.fn(
      async (listener: (snapshot: LifecycleSnapshot) => void) => {
        lifecycleListener = listener;
        return () => {
          if (lifecycleListener === listener) lifecycleListener = null;
        };
      },
    ),
  };
});
vi.mock("../ipc/uiCheckpoint", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../ipc/uiCheckpoint")>();
  return {
    ...actual,
    getUiCheckpoint: vi.fn().mockResolvedValue(null),
    saveUiCheckpoint: vi.fn().mockImplementation(
      async (input: checkpointIpc.SaveUiCheckpointInput) => ({
        schemaVersion: 2 as const,
        updatedAt: 1,
        section: input.section,
        flashcardsWorkspace: input.flashcardsWorkspace ?? null,
        deckId: input.deckId ?? null,
        editorDraftId: input.editorDraftId ?? null,
        practice: input.practice ?? null,
        analytics: input.analytics ?? null,
      }),
    ),
  };
});

function Harness() {
  const controller = useAppController();
  const target = controller.practiceWorkflow?.returnTarget ?? null;
  if (controller.isLoading) return <span>Loading</span>;
  return (
    <div>
      <output aria-label="section">{controller.section}</output>
      <output aria-label="workflow">{controller.activeWorkflow ?? "none"}</output>
      <output aria-label="route request">{controller.flashcardsRoute.requestId}</output>
      <output aria-label="route view">
        {controller.flashcardsRoute.view === "deck"
          ? controller.flashcardsRoute.deckId
          : controller.flashcardsRoute.workspace}
      </output>
      <output aria-label="return target">
        {target
          ? `${target.section}|${target.flashcardsWorkspace ?? ""}|${target.deckId ?? ""}`
          : "none"}
      </output>
      <output aria-label="analytics request">
        {controller.analyticsLaunch?.requestId ?? "none"}
      </output>
      <output aria-label="analytics search launch">
        {controller.flashcardsRoute.view === "root" &&
        controller.flashcardsRoute.analyticsSearchLaunch
          ? `${controller.flashcardsRoute.analyticsSearchLaunch.requestId}|${controller.flashcardsRoute.analyticsSearchLaunch.source}|${controller.flashcardsRoute.analyticsSearchLaunch.tagIds.join(",")}`
          : "none"}
      </output>
      <output aria-label="library section">{controller.librarySection}</output>
      <output aria-label="subject page">{controller.homeSubjectId ?? "none"}</output>
      <output aria-label="finished sessions">{controller.finishedPracticeSessions}</output>
      <output aria-label="subject focus">
        {controller.flashcardsRoute.view === "root" && controller.flashcardsRoute.subjectFocus
          ? controller.flashcardsRoute.subjectFocus.subjectId ?? "no subject"
          : "none"}
      </output>
      <output aria-label="restored selection">
        {controller.flashcardsRoute.view === "root"
          ? controller.flashcardsRoute.restoreDeckSelection?.map((entry) => entry.id).join(",") ?? "none"
          : "none"}
      </output>
      <button type="button" onClick={() => void controller.navigate("study-guides")}>
        Open Study Guides
      </button>
      <button type="button" onClick={() => void controller.navigate("decks")}>
        Open Decks
      </button>
      <button type="button" onClick={() => void controller.navigate("documents")}>
        Open Documents
      </button>
      <button type="button" onClick={() => void controller.navigate("review")}>
        Open Review
      </button>
      <button type="button" onClick={() => void controller.navigate("home")}>
        Open Home
      </button>
      <button type="button" onClick={() => void controller.openSearch()}>
        Open Search
      </button>
      <button type="button" onClick={() => void controller.openSubject("c".repeat(32))}>
        Open subject
      </button>
      <button type="button" onClick={() => void controller.openSubject(null)}>
        Open No subject
      </button>
      <button type="button" onClick={() => void controller.openSubject(null, "documents")}>
        Open No subject documents
      </button>
      <button type="button" onClick={() => void controller.openLibrary()}>
        Open Library
      </button>
      <button
        type="button"
        onClick={() => void controller.openLibrary({ subjectId: "c".repeat(32) })}
      >
        Open Library for subject
      </button>
      <button type="button" onClick={() => void controller.openLibrary({ subjectId: null })}>
        Open Library for No subject
      </button>
      <button type="button" onClick={() => void controller.openLibrary({ deckId: "d".repeat(32) })}>
        Open Library at deck
      </button>
      <button type="button" onClick={() => void controller.practiceScope(["d".repeat(32)], [])}>
        Start review
      </button>
      <button
        type="button"
        onClick={() => controller.openAnalyticsTagSearch("analytics-tag-id")}
      >
        Open Analytics Search
      </button>
      <button
        type="button"
        onClick={() => void controller.navigateFlashcardsWorkspace("search")}
      >
        Open Search workspace
      </button>
      <button
        type="button"
        onClick={() =>
          controller.openFlashcardsDeck("d".repeat(32), [
            { id: "d".repeat(32), revision: 1 },
          ])
        }
      >
        Open deck
      </button>
      <button type="button" onClick={controller.returnToFlashcardsLibrary}>
        Return to library
      </button>
      <button type="button" onClick={() => void controller.practiceDeck("launch-deck")}>
        Open Practice
      </button>
      <button
        type="button"
        onClick={() =>
          void controller.practiceSelected(
            {
              deckIds: ["d".repeat(32), "e".repeat(32)],
              tagIds: ["t".repeat(32)],
              shuffle: false,
              mode: "extra",
            },
            [
              { id: "d".repeat(32), revision: 1 },
              { id: "e".repeat(32), revision: 1 },
            ],
          )
        }
      >
        Open selected Practice
      </button>
      <button
        type="button"
        onClick={() =>
          controller.reportPracticeCheckpoint({
            sessionId: "s".repeat(32),
            position: 9,
            revealed: true,
          })
        }
      >
        Report cursor
      </button>
      <button type="button" onClick={() => controller.reportPracticeCheckpoint(null)}>
        Report summary
      </button>
      <button
        type="button"
        onClick={() => {
          if (target) controller.returnFromPractice(target);
        }}
      >
        End Practice
      </button>
    </div>
  );
}

function renderHarness() {
  return render(
    <OverlayProvider>
      <Harness />
    </OverlayProvider>,
  );
}

async function openDecks(): Promise<void> {
  await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("home"));
  fireEvent.click(screen.getByRole("button", { name: "Open Decks" }));
  await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("decks"));
}

function lifecycle(phase: LifecycleSnapshot["phase"]): void {
  if (!lifecycleListener) throw new Error("Lifecycle listener is not registered.");
  lifecycleListener({ schemaVersion: 1, phase, observedAtUtcMillis: 10 });
}

describe("application checkpoint lifecycle", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    lifecycleListener = null;
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck({ id: "d".repeat(32) })]);
    vi.mocked(checkpointIpc.getUiCheckpoint).mockResolvedValue(null);
  });

  it("saves Study Guides as a visible destination", async () => {
    renderHarness();
    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Open Study Guides" }));
    await waitFor(() =>
      expect(screen.getByLabelText("section").textContent).toBe("study-guides"),
    );
    await waitFor(() =>
      expect(checkpointIpc.saveUiCheckpoint).toHaveBeenCalledWith(
        expect.objectContaining({ section: "study-guides" }),
      ),
    );
  });

  it("checkpoints only the visible Search workspace and omits the Analytics launch", async () => {
    renderHarness();
    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Open Analytics Search" }));
    await waitFor(() =>
      expect(screen.getByLabelText("analytics search launch").textContent).toMatch(
        /^\d+\|analytics\|analytics-tag-id$/u,
      ),
    );
    vi.mocked(checkpointIpc.saveUiCheckpoint).mockClear();

    lifecycle("background");

    await waitFor(() => expect(checkpointIpc.saveUiCheckpoint).toHaveBeenCalledOnce());
    const saved = vi.mocked(checkpointIpc.saveUiCheckpoint).mock.calls[0]?.[0];
    expect(saved).toEqual({
      section: "decks",
      flashcardsWorkspace: "search",
      deckId: undefined,
      practice: undefined,
      analytics: undefined,
    });
    expect(JSON.stringify(saved)).not.toContain("analytics-tag-id");
    expect(JSON.stringify(saved)).not.toContain("requestId");
    expect(JSON.stringify(saved)).not.toContain("source");
  });

  it("constructs plain routes for top-level, workspace, deck, and library navigation", async () => {
    renderHarness();
    await openDecks();

    fireEvent.click(screen.getByRole("button", { name: "Open Analytics Search" }));
    expect(screen.getByLabelText("analytics search launch").textContent).not.toBe("none");
    fireEvent.click(screen.getByRole("button", { name: "Open Search workspace" }));
    await waitFor(() =>
      expect(screen.getByLabelText("analytics search launch").textContent).toBe("none"),
    );

    fireEvent.click(screen.getByRole("button", { name: "Open Analytics Search" }));
    fireEvent.click(screen.getByRole("button", { name: "Open deck" }));
    expect(screen.getByLabelText("analytics search launch").textContent).toBe("none");
    vi.mocked(checkpointIpc.saveUiCheckpoint).mockClear();
    lifecycle("background");
    await waitFor(() => expect(checkpointIpc.saveUiCheckpoint).toHaveBeenCalledOnce());
    expect(
      JSON.stringify(vi.mocked(checkpointIpc.saveUiCheckpoint).mock.calls[0]?.[0]),
    ).not.toContain("returnDeckSelection");

    fireEvent.click(screen.getByRole("button", { name: "Open Analytics Search" }));
    fireEvent.click(screen.getByRole("button", { name: "Return to library" }));
    await waitFor(() =>
      expect(screen.getByLabelText("analytics search launch").textContent).toBe("none"),
    );

    fireEvent.click(screen.getByRole("button", { name: "Open Analytics Search" }));
    fireEvent.click(screen.getByRole("button", { name: "Open Study Guides" }));
    await waitFor(() =>
      expect(screen.getByLabelText("analytics search launch").textContent).toBe("none"),
    );
    expect(screen.getByLabelText("section").textContent).toBe("study-guides");
  });

  it("flushes the latest active Practice cursor and logical return target on lifecycle save", async () => {
    renderHarness();
    await waitFor(() => expect(lifecycleListener).not.toBeNull());
    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Open Practice" }));
    await waitFor(() => expect(screen.getByLabelText("workflow").textContent).toBe("practice"));
    fireEvent.click(screen.getByRole("button", { name: "Report cursor" }));
    vi.mocked(checkpointIpc.saveUiCheckpoint).mockClear();

    lifecycle("background");

    await waitFor(() =>
      expect(checkpointIpc.saveUiCheckpoint).toHaveBeenCalledWith({
        section: "decks",
        flashcardsWorkspace: "decks",
        deckId: undefined,
        practice: {
          sessionId: "s".repeat(32),
          position: 9,
          revealed: true,
          returnTarget: {
            section: "decks",
            flashcardsWorkspace: "decks",
            deckId: null,
          },
        },
        analytics: undefined,
      }),
    );
    lifecycle("resuming");
    expect(checkpointIpc.saveUiCheckpoint).toHaveBeenCalledOnce();
  });

  it("keeps selected-deck return state in memory, out of the checkpoint, and reconciles it on return", async () => {
    const current = makeDeck({ id: "d".repeat(32), revision: 1, generatedCardCount: 0 });
    const removed = makeDeck({ id: "e".repeat(32), revision: 1, generatedCardCount: 2 });
    vi.mocked(decksIpc.listDecks)
      .mockResolvedValueOnce([current, removed])
      .mockResolvedValueOnce([current]);
    renderHarness();
    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Open selected Practice" }));
    await waitFor(() => expect(screen.getByLabelText("workflow").textContent).toBe("practice"));
    fireEvent.click(screen.getByRole("button", { name: "Report cursor" }));
    vi.mocked(checkpointIpc.saveUiCheckpoint).mockClear();
    lifecycle("background");
    await waitFor(() => expect(checkpointIpc.saveUiCheckpoint).toHaveBeenCalled());
    const saved = vi.mocked(checkpointIpc.saveUiCheckpoint).mock.calls.at(-1)?.[0];
    expect(JSON.stringify(saved)).not.toContain("restoreDeckSelection");
    expect(JSON.stringify(saved)).not.toContain("e".repeat(32));

    fireEvent.click(screen.getByRole("button", { name: "End Practice" }));
    await waitFor(() =>
      expect(screen.getByLabelText("restored selection").textContent).toBe(current.id),
    );
  });

  it("counts a session that reaches its summary once, so Home can reread what is due", async () => {
    renderHarness();
    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Open Practice" }));
    await waitFor(() => expect(screen.getByLabelText("workflow").textContent).toBe("practice"));
    // Before the session has a cursor, an empty report is not a finished session.
    fireEvent.click(screen.getByRole("button", { name: "Report summary" }));
    expect(screen.getByLabelText("finished sessions").textContent).toBe("0");

    fireEvent.click(screen.getByRole("button", { name: "Report cursor" }));
    fireEvent.click(screen.getByRole("button", { name: "Report summary" }));
    expect(screen.getByLabelText("finished sessions").textContent).toBe("1");
    fireEvent.click(screen.getByRole("button", { name: "Report summary" }));
    expect(screen.getByLabelText("finished sessions").textContent).toBe("1");
    expect(screen.getByLabelText("workflow").textContent).toBe("practice");
  });

  it("opens a fresh library on Home and saves it", async () => {
    renderHarness();
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("home"));
    await waitFor(() =>
      expect(checkpointIpc.saveUiCheckpoint).toHaveBeenCalledWith(
        expect.objectContaining({ section: "home" }),
      ),
    );
    const saved = vi
      .mocked(checkpointIpc.saveUiCheckpoint)
      .mock.calls.find(([input]) => input.section === "home")?.[0];
    expect(saved?.flashcardsWorkspace).toBeUndefined();
    expect(saved?.deckId).toBeUndefined();
  });

  it("saves Home as the return target of a session started there", async () => {
    renderHarness();
    await waitFor(() => expect(lifecycleListener).not.toBeNull());
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("home"));
    fireEvent.click(screen.getByRole("button", { name: "Start review" }));
    await waitFor(() => expect(screen.getByLabelText("workflow").textContent).toBe("practice"));
    expect(screen.getByLabelText("return target").textContent).toBe("home||");
    fireEvent.click(screen.getByRole("button", { name: "Report cursor" }));
    vi.mocked(checkpointIpc.saveUiCheckpoint).mockClear();

    lifecycle("background");

    await waitFor(() =>
      expect(checkpointIpc.saveUiCheckpoint).toHaveBeenCalledWith(
        expect.objectContaining({
          section: "home",
          practice: {
            sessionId: "s".repeat(32),
            position: 9,
            revealed: true,
            returnTarget: { section: "home", flashcardsWorkspace: null, deckId: null },
          },
        }),
      ),
    );
    fireEvent.click(screen.getByRole("button", { name: "End Practice" }));
    expect(screen.getByLabelText("section").textContent).toBe("home");
  });

  it("keeps a running session when Review is chosen and returns a Review session to Review", async () => {
    renderHarness();
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("home"));
    fireEvent.click(screen.getByRole("button", { name: "Open Review" }));
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("review"));
    fireEvent.click(screen.getByRole("button", { name: "Start review" }));
    await waitFor(() => expect(screen.getByLabelText("workflow").textContent).toBe("practice"));
    expect(screen.getByLabelText("return target").textContent).toBe("review||");

    fireEvent.click(screen.getByRole("button", { name: "Open Review" }));
    await waitFor(() => expect(screen.getByLabelText("workflow").textContent).toBe("practice"));

    fireEvent.click(screen.getByRole("button", { name: "End Practice" }));
    expect(screen.getByLabelText("section").textContent).toBe("review");
    expect(screen.getByLabelText("workflow").textContent).toBe("none");
  });

  it("restores a session saved behind Review and returns there", async () => {
    vi.mocked(checkpointIpc.getUiCheckpoint).mockResolvedValue({
      schemaVersion: 2,
      updatedAt: 1,
      section: "review",
      flashcardsWorkspace: null,
      deckId: null,
      editorDraftId: null,
      practice: {
        sessionId: "s".repeat(32),
        position: 2,
        revealed: true,
        returnTarget: { section: "review", flashcardsWorkspace: null, deckId: null },
      },
      analytics: null,
    });

    renderHarness();

    await waitFor(() => expect(screen.getByLabelText("workflow").textContent).toBe("practice"));
    expect(screen.getByLabelText("return target").textContent).toBe("review||");
    fireEvent.click(screen.getByRole("button", { name: "End Practice" }));
    expect(screen.getByLabelText("section").textContent).toBe("review");
  });

  it("returns to Home on platform Back and lets Home itself leave the app", async () => {
    renderHarness();
    await openDecks();
    await waitFor(() => expect(dispatchPlatformBack()).toBe(true));
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("home"));
    await waitFor(() => expect(dispatchPlatformBack()).toBe(false));
  });

  it("opens Search/Filter and subjects inside Library and remembers the Library view", async () => {
    renderHarness();
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("home"));
    expect(screen.getByLabelText("library section").textContent).toBe("decks");

    fireEvent.click(screen.getByRole("button", { name: "Open Search" }));
    await waitFor(() => expect(screen.getByLabelText("route view").textContent).toBe("search"));
    expect(screen.getByLabelText("section").textContent).toBe("decks");

    fireEvent.click(screen.getByRole("button", { name: "Open subject" }));
    await waitFor(() =>
      expect(screen.getByLabelText("subject page").textContent).toBe("c".repeat(32)),
    );
    expect(screen.getByLabelText("section").textContent).toBe("home");
    fireEvent.click(screen.getByRole("button", { name: "Open No subject" }));
    await waitFor(() =>
      expect(screen.getByLabelText("subject focus").textContent).toBe("no subject"),
    );

    fireEvent.click(screen.getByRole("button", { name: "Open No subject documents" }));
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("documents"));
    expect(screen.getByLabelText("library section").textContent).toBe("documents");
    fireEvent.click(screen.getByRole("button", { name: "Open Home" }));
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("home"));
    expect(screen.getByLabelText("library section").textContent).toBe("documents");

    vi.mocked(checkpointIpc.saveUiCheckpoint).mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Open Library for subject" }));
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("decks"));
    fireEvent.click(screen.getByRole("button", { name: "Open subject" }));
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("home"));
    lifecycle("background");
    await waitFor(() => expect(checkpointIpc.saveUiCheckpoint).toHaveBeenCalled());
    // The focus request is process-local, like an Analytics launch.
    for (const [saved] of vi.mocked(checkpointIpc.saveUiCheckpoint).mock.calls) {
      // The subject page is process-local too: Home is saved, never the subject.
      expect(JSON.stringify(saved)).not.toContain("subjectFocus");
      expect(JSON.stringify(saved)).not.toContain("c".repeat(32));
    }
  });

  it("opens Library as last left, scoped to a subject, or at a deck", async () => {
    renderHarness();
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("home"));

    fireEvent.click(screen.getByRole("button", { name: "Open Library" }));
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("decks"));
    fireEvent.click(screen.getByRole("button", { name: "Open Documents" }));
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("documents"));
    fireEvent.click(screen.getByRole("button", { name: "Open Home" }));
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("home"));
    fireEvent.click(screen.getByRole("button", { name: "Open Library" }));
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("documents"));

    fireEvent.click(screen.getByRole("button", { name: "Open Library for subject" }));
    await waitFor(() =>
      expect(screen.getByLabelText("subject focus").textContent).toBe("c".repeat(32)),
    );
    expect(screen.getByLabelText("section").textContent).toBe("decks");
    expect(screen.getByLabelText("route view").textContent).toBe("decks");

    fireEvent.click(screen.getByRole("button", { name: "Open Library for No subject" }));
    await waitFor(() =>
      expect(screen.getByLabelText("subject focus").textContent).toBe("no subject"),
    );

    fireEvent.click(screen.getByRole("button", { name: "Open Home" }));
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("home"));
    fireEvent.click(screen.getByRole("button", { name: "Open Library at deck" }));
    await waitFor(() =>
      expect(screen.getByLabelText("route view").textContent).toBe("d".repeat(32)),
    );
    expect(screen.getByLabelText("section").textContent).toBe("decks");
  });

  it("restores Study Guides behind a resumable session and returns there", async () => {
    vi.mocked(checkpointIpc.getUiCheckpoint).mockResolvedValue({
      schemaVersion: 2,
      updatedAt: 1,
      section: "study-guides",
      flashcardsWorkspace: null,
      deckId: null,
      editorDraftId: null,
      practice: {
        sessionId: "s".repeat(32),
        position: 3,
        revealed: false,
        returnTarget: {
          section: "study-guides",
          flashcardsWorkspace: null,
          deckId: null,
        },
      },
      analytics: null,
    });

    renderHarness();

    await waitFor(() => expect(screen.getByLabelText("workflow").textContent).toBe("practice"));
    expect(screen.getByLabelText("return target").textContent).toBe("study-guides||");
    fireEvent.click(screen.getByRole("button", { name: "End Practice" }));
    expect(screen.getByLabelText("section").textContent).toBe("study-guides");
    expect(screen.getByLabelText("workflow").textContent).toBe("none");
  });

  it("falls back from a deleted deck without discarding a resumable session", async () => {
    vi.mocked(checkpointIpc.getUiCheckpoint).mockResolvedValue({
      schemaVersion: 2,
      updatedAt: 1,
      section: "decks",
      flashcardsWorkspace: "decks",
      deckId: "missing-deck",
      editorDraftId: null,
      practice: {
        sessionId: "s".repeat(32),
        position: 3,
        revealed: false,
        returnTarget: {
          section: "decks",
          flashcardsWorkspace: "decks",
          deckId: "missing-deck",
        },
      },
      analytics: null,
    });

    renderHarness();

    await waitFor(() => expect(screen.getByLabelText("workflow").textContent).toBe("practice"));
    expect(screen.getByLabelText("route view").textContent).toBe("decks");
    expect(screen.getByLabelText("return target").textContent).toBe("decks|decks|");
  });

  it("allocates a fresh route request when returning to a restored deck target", async () => {
    const deck = makeDeck({ id: "e".repeat(32) });
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);
    vi.mocked(checkpointIpc.getUiCheckpoint).mockResolvedValue({
      schemaVersion: 2,
      updatedAt: 1,
      section: "decks",
      flashcardsWorkspace: "decks",
      deckId: deck.id,
      editorDraftId: null,
      practice: {
        sessionId: "s".repeat(32),
        position: 3,
        revealed: false,
        returnTarget: {
          section: "decks",
          flashcardsWorkspace: "decks",
          deckId: deck.id,
        },
      },
      analytics: null,
    });
    renderHarness();
    await waitFor(() => expect(screen.getByLabelText("workflow").textContent).toBe("practice"));
    const restoredRequest = screen.getByLabelText("route request").textContent;

    fireEvent.click(screen.getByRole("button", { name: "End Practice" }));

    await waitFor(() =>
      expect(screen.getByLabelText("route view").textContent).toBe(deck.id),
    );
    expect(screen.getByLabelText("route request").textContent).not.toBe(restoredRequest);
  });

  it("restores Search with a fresh route request after Practice", async () => {
    vi.mocked(checkpointIpc.getUiCheckpoint).mockResolvedValue({
      schemaVersion: 2,
      updatedAt: 1,
      section: "decks",
      flashcardsWorkspace: "search",
      deckId: null,
      editorDraftId: null,
      practice: {
        sessionId: "s".repeat(32),
        position: 3,
        revealed: false,
        returnTarget: {
          section: "decks",
          flashcardsWorkspace: "search",
          deckId: null,
        },
      },
      analytics: null,
    });
    renderHarness();
    await waitFor(() => expect(screen.getByLabelText("workflow").textContent).toBe("practice"));
    const restoredRequest = screen.getByLabelText("route request").textContent;

    fireEvent.click(screen.getByRole("button", { name: "End Practice" }));

    await waitFor(() =>
      expect(screen.getByLabelText("route view").textContent).toBe("search"),
    );
    expect(screen.getByLabelText("route request").textContent).not.toBe(restoredRequest);
  });

  it("reconstructs a fresh Analytics request after restored Practice", async () => {
    vi.mocked(checkpointIpc.getUiCheckpoint).mockResolvedValue({
      schemaVersion: 2,
      updatedAt: 1,
      section: "analytics",
      flashcardsWorkspace: null,
      deckId: null,
      editorDraftId: null,
      practice: {
        sessionId: "s".repeat(32),
        position: 3,
        revealed: false,
        returnTarget: {
          section: "analytics",
          flashcardsWorkspace: null,
          deckId: null,
        },
      },
      analytics: {
        range: "30d",
        deckIds: ["d".repeat(32)],
        tagIds: [],
      },
    });
    renderHarness();
    await waitFor(() => expect(screen.getByLabelText("workflow").textContent).toBe("practice"));
    const restoredRequest = screen.getByLabelText("analytics request").textContent;

    fireEvent.click(screen.getByRole("button", { name: "End Practice" }));

    expect(screen.getByLabelText("section").textContent).toBe("analytics");
    expect(screen.getByLabelText("analytics request").textContent).not.toBe(restoredRequest);
  });
});

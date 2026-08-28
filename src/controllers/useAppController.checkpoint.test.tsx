import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as decksIpc from "../ipc/decks";
import type { LifecycleSnapshot } from "../ipc/platform";
import * as settingsIpc from "../ipc/settings";
import * as checkpointIpc from "../ipc/uiCheckpoint";
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
      <button type="button" onClick={() => void controller.navigate("study-guides")}>
        Open Study Guides
      </button>
      <button type="button" onClick={() => void controller.practiceDeck("launch-deck")}>
        Open Practice
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
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("decks"));
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

  it("flushes the latest active Practice cursor and logical return target on lifecycle save", async () => {
    renderHarness();
    await waitFor(() => expect(lifecycleListener).not.toBeNull());
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

    expect(screen.getByLabelText("route view").textContent).toBe(deck.id);
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

    expect(screen.getByLabelText("route view").textContent).toBe("search");
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

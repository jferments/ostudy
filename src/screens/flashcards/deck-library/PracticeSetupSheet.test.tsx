import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import * as practiceIpc from "../../../ipc/practice";
import * as tagsIpc from "../../../ipc/tags";
import { dispatchPlatformBack } from "../../../platform/backNavigation";
import { deferred } from "../../../test/factories";
import { LayoutModeProvider } from "../../../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../../../ui/overlays/OverlayProvider";
import { PracticeSetupSheet } from "./PracticeSetupSheet";
import type { PracticeDeckScope } from "./practiceSetupModel";

vi.mock("../../../ipc/practice", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../../ipc/practice")>();
  return { ...actual, getPracticeWorkload: vi.fn() };
});
vi.mock("../../../ipc/tags", () => ({ listTags: vi.fn() }));

const deck: DeckSummary = {
  id: "a".repeat(32),
  name: "Anatomy",
  description: "",
  sourceNoteCount: 5,
  generatedCardCount: 6,
  studyableDueCount: 2,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 1,
  revision: 3,
};
const scope: PracticeDeckScope = {
  deckIds: [deck.id],
  decks: [
    {
      id: deck.id,
      name: deck.name,
      revision: deck.revision,
      generatedCardCount: deck.generatedCardCount,
    },
  ],
};
const workload: practiceIpc.PracticeWorkloadOverview = {
  newLimit: 20,
  reviewLimit: 200,
  newCompleted: 1,
  reviewsCompleted: 2,
  newRemaining: 19,
  reviewsRemaining: 198,
  studyableDueReviews: 5,
  studyableDueNew: 3,
  scheduledReviews: 0,
  scheduledNew: 0,
  omittedDueReviews: 5,
  omittedDueNew: 3,
  needsMorePractice: 1,
  extraAvailable: 8,
};
const tag = {
  id: "b".repeat(32),
  name: "Bones",
  sourceNoteCount: 2,
  generatedCardCount: 3,
  studyableDueCount: 1,
};

function renderSheet(
  overrides: Partial<React.ComponentProps<typeof PracticeSetupSheet>> = {},
) {
  const props: React.ComponentProps<typeof PracticeSetupSheet> = {
    scope,
    currentDecks: [deck],
    initialShuffle: true,
    onClose: vi.fn(),
    onStart: vi.fn(),
    ...overrides,
  };
  return {
    props,
    ...render(
      <LayoutModeProvider>
        <OverlayProvider>
          <PracticeSetupSheet {...props} />
        </OverlayProvider>
      </LayoutModeProvider>,
    ),
  };
}

describe("PracticeSetupSheet", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.matchMedia = vi.fn().mockReturnValue({
      matches: false,
      media: "",
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    });
    vi.mocked(tagsIpc.listTags).mockResolvedValue([tag]);
    vi.mocked(practiceIpc.getPracticeWorkload).mockResolvedValue(workload);
  });

  it("shows the snapshot and authoritative counts without substituting extra study", async () => {
    renderSheet();
    const dialog = await screen.findByRole("dialog", { name: "Practice selected decks" });
    expect(within(dialog).getByText("Anatomy")).toBeTruthy();
    expect(
      within(dialog).getByText(/matching cards must have at least one selected tag/u),
    ).toBeTruthy();
    expect(within(dialog).getByText("Due reviews")).toBeTruthy();
    expect(within(dialog).getByText("Omitted new")).toBeTruthy();
    const scheduled = within(dialog).getByRole("radio", { name: /Scheduled study/u });
    const extra = within(dialog).getByRole("radio", { name: /Extra study/u });
    expect((scheduled as HTMLInputElement).checked).toBe(true);
    expect((scheduled as HTMLInputElement).disabled).toBe(true);
    expect((extra as HTMLInputElement).checked).toBe(false);
    expect(within(dialog).getByRole("button", { name: "Start practice" }).hasAttribute("disabled")).toBe(true);
  });

  it("launches the exact displayed tag, shuffle, and explicit extra mode once", async () => {
    const onStart = vi.fn().mockResolvedValue(undefined);
    renderSheet({ onStart });
    const tagChoice = await screen.findByRole("checkbox", { name: /Bones/u });
    fireEvent.click(tagChoice);
    await waitFor(() =>
      expect(practiceIpc.getPracticeWorkload).toHaveBeenLastCalledWith(
        [deck.id],
        [tag.id],
      ),
    );
    fireEvent.click(screen.getByRole("radio", { name: /Extra study/u }));
    fireEvent.click(screen.getByRole("checkbox", { name: /Shuffle cards/u }));
    expect(screen.getByText(/1 deck, 1 tag, extra mode, shuffle off/u)).toBeTruthy();
    const start = screen.getByRole("button", { name: "Start practice" });
    fireEvent.click(start);
    fireEvent.click(start);
    await waitFor(() =>
      expect(onStart).toHaveBeenCalledWith({
        deckIds: [deck.id],
        tagIds: [tag.id],
        shuffle: false,
        mode: "extra",
      }),
    );
    expect(onStart).toHaveBeenCalledTimes(1);
  });

  it("distinguishes workload failure from a true zero and supports retry", async () => {
    vi.mocked(practiceIpc.getPracticeWorkload)
      .mockRejectedValueOnce(new Error("Planner unavailable"))
      .mockRejectedValueOnce(new Error("Planner unavailable"))
      .mockResolvedValueOnce(workload);
    renderSheet();
    expect(await screen.findByText(/Study counts are unavailable: Planner unavailable/u)).toBeTruthy();
    expect(screen.queryByText("Due reviews")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Retry study counts" }));
    expect(await screen.findByText("Due reviews")).toBeTruthy();
  });

  it("rejects an invalidated snapshot with clear reselect guidance", async () => {
    renderSheet({ currentDecks: [{ ...deck, revision: deck.revision + 1 }] });
    expect(await screen.findByText(/changed while setup was open/u)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Start practice" }).hasAttribute("disabled")).toBe(true);
    expect(practiceIpc.getPracticeWorkload).not.toHaveBeenCalled();
    expect(screen.queryByText("Loading tags…")).toBeNull();
    expect(screen.queryByText("Updating…")).toBeNull();
    expect(screen.queryByText("Due reviews")).toBeNull();
    expect(screen.queryByRole("radio", { name: /Scheduled study/u })).toBeNull();
  });

  it("synchronously hides an obsolete workload until the exact tagged query resolves", async () => {
    const unfiltered = deferred<practiceIpc.PracticeWorkloadOverview>();
    const tagged = deferred<practiceIpc.PracticeWorkloadOverview>();
    vi.mocked(practiceIpc.getPracticeWorkload).mockImplementation(async (_deckIds, tagIds) =>
      !tagIds || tagIds.length === 0 ? unfiltered.promise : tagged.promise,
    );
    const onStart = vi.fn();
    renderSheet({ onStart });
    unfiltered.resolve({ ...workload, scheduledReviews: 4, scheduledNew: 0 });
    await screen.findByText(/1 deck, 0 tags, scheduled mode/u);

    fireEvent.click(await screen.findByRole("checkbox", { name: /Bones/u }));
    expect(screen.queryByText(/1 deck, 0 tags, scheduled mode/u)).toBeNull();
    expect(screen.queryByText("Due reviews")).toBeNull();
    expect(screen.getByRole("button", { name: "Start practice" }).hasAttribute("disabled")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Start practice" }));
    expect(onStart).not.toHaveBeenCalled();

    tagged.resolve({ ...workload, scheduledReviews: 7, scheduledNew: 0 });
    await screen.findByText(/1 deck, 1 tag, scheduled mode/u);
    fireEvent.click(screen.getByRole("button", { name: "Start practice" }));
    expect(onStart).toHaveBeenCalledWith({
      deckIds: [deck.id],
      tagIds: [tag.id],
      shuffle: true,
      mode: "scheduled",
    });
  });

  it("clears ready setup data when a valid snapshot becomes stale", async () => {
    const rendered = renderSheet();
    await screen.findByText("Due reviews");
    rendered.rerender(
      <LayoutModeProvider>
        <OverlayProvider>
          <PracticeSetupSheet
            {...rendered.props}
            currentDecks={[{ ...deck, revision: deck.revision + 1 }]}
          />
        </OverlayProvider>
      </LayoutModeProvider>,
    );
    expect(await screen.findByText(/changed while setup was open/u)).toBeTruthy();
    expect(screen.queryByText("Due reviews")).toBeNull();
    expect(screen.queryByText("Loading tags…")).toBeNull();
    expect(screen.queryByText("Updating…")).toBeNull();
  });

  it("opens compact setup at the top, closes with Escape, and restores the opener", async () => {
    window.matchMedia = vi.fn((query: string) => ({
      matches: query.includes("760"),
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));
    function Harness() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>Open setup</button>
          {open ? (
            <PracticeSetupSheet
              scope={scope}
              currentDecks={[deck]}
              initialShuffle
              onClose={() => setOpen(false)}
              onStart={vi.fn()}
            />
          ) : null}
        </>
      );
    }
    render(<LayoutModeProvider><OverlayProvider><Harness /></OverlayProvider></LayoutModeProvider>);
    const opener = screen.getByRole("button", { name: "Open setup" });
    opener.focus();
    fireEvent.click(opener);
    const dialog = await screen.findByRole("dialog");
    const close = screen.getByRole("button", { name: "Close dialog" });
    const dialogBody = dialog.querySelector("[data-layout-mode]")?.parentElement;
    expect(close.hasAttribute("disabled")).toBe(false);
    await waitFor(() => expect(document.activeElement).toBe(close));
    expect(dialogBody?.scrollTop).toBe(0);
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(opener));
  });

  it("refuses every dismissal and competing retry while start is busy", async () => {
    const pendingStart = deferred<void>();
    vi.mocked(tagsIpc.listTags).mockRejectedValue(new Error("Tags unavailable"));
    const onClose = vi.fn();
    renderSheet({ onClose, onStart: () => pendingStart.promise });
    const extra = await screen.findByRole("radio", { name: /Extra study/u });
    fireEvent.click(extra);
    fireEvent.click(screen.getByRole("button", { name: "Start practice" }));
    await waitFor(() => expect(screen.getByRole("dialog").getAttribute("aria-busy")).toBe("true"));
    const retry = screen.getByRole("button", { name: "Retry tags" });
    expect(retry.hasAttribute("disabled")).toBe(true);
    fireEvent.click(retry);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(dispatchPlatformBack()).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Close dialog" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onClose).not.toHaveBeenCalled();
    expect(tagsIpc.listTags).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("dialog")).toBeTruthy();
    pendingStart.resolve();
  });

  it("exposes explicit wide, medium, and compact setup layouts", async () => {
    for (const [mode, width] of [["wide", 1400], ["medium", 900], ["compact", 390]] as const) {
      window.matchMedia = vi.fn((query: string) => ({
        matches: query.includes("760") ? width <= 760 : query.includes("1180") ? width <= 1180 : false,
        media: query,
        onchange: null,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        addListener: vi.fn(),
        removeListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }));
      renderSheet();
      const content = (await screen.findByRole("dialog")).querySelector("[data-layout-mode]");
      expect(content?.getAttribute("data-layout-mode")).toBe(mode);
      cleanup();
    }
  });
});

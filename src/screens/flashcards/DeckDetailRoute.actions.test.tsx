import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ToastAction } from "../../components/Toast";
import type { DeckSummary } from "../../ipc/decks";
import { DeckDetailRoute } from "./DeckDetailRoute";

const calls = vi.hoisted(() => ({
  deck: vi.fn(), setArchived: vi.fn(), deleteDeck: vi.fn(), undo: vi.fn(),
  confirm: vi.fn(), invalidate: vi.fn(),
}));
vi.mock("../../controllers/decksController", () => ({
  DecksController: class {
    deck = calls.deck; setArchived = calls.setArchived; deleteDeck = calls.deleteDeck;
    undo = calls.undo; invalidate = calls.invalidate;
  },
}));
vi.mock("../../ui/overlays/overlayContext", () => ({ useConfirm: () => calls.confirm }));
vi.mock("../../components/SourceCitationPanel", () => ({ SourceCitationPanel: () => null }));
vi.mock("../cards/CardPracticeContext", () => ({ useCardPracticeLauncher: () => vi.fn() }));
vi.mock("../cards/CardBrowserWorkspace", () => ({
  CardBrowserWorkspace: ({ fixedDeckId, showFilters }: { fixedDeckId: string; showFilters: boolean }) =>
    <section aria-label="Shared deck browser" data-deck-id={fixedDeckId} data-filters={String(showFilters)} />,
}));
vi.mock("./DeckTransferDialog", () => ({
  DeckTransferDialog: ({ request }: { request: { kind: string; decks: DeckSummary[] } }) =>
    <section role="dialog" aria-label="Export selected deck">{request.decks[0].name}</section>,
}));

const deck: DeckSummary = {
  id: "a".repeat(32), name: "Study examples", description: "A populated test deck",
  sourceNoteCount: 2, generatedCardCount: 4, studyableDueCount: 0,
  archivedAt: null, createdAt: 1, updatedAt: 1, revision: 3,
};
function mount(selected = deck) {
  calls.deck.mockResolvedValue(selected);
  const refresh = vi.fn().mockResolvedValue([selected]);
  const back = vi.fn().mockResolvedValue(undefined);
  const toast = vi.fn();
  const practice = vi.fn();
  render(<DeckDetailRoute decks={[selected]} refreshDecks={refresh} onPracticeDeck={practice}
    openRequest={{ requestId: 1, deckId: selected.id }} onReturnToLibrary={back}
    onDirtyChange={vi.fn()} showToast={toast} />);
  return { refresh, back, toast, practice };
}
beforeEach(() => {
  vi.resetAllMocks();
  calls.confirm.mockResolvedValue(true);
  calls.deleteDeck.mockResolvedValue({ undoToken: "undo-token", undoExpiresAt: Date.now() + 60_000 });
  calls.undo.mockResolvedValue({ restoredCount: 4 });
  calls.setArchived.mockResolvedValue({ ...deck, archivedAt: 100, revision: 4 });
});

describe("production deck route action contracts", () => {
  it("delegates deck-only browsing and practice using the original deck identity", async () => {
    const { practice } = mount();
    const browser = screen.getByRole("region", { name: "Shared deck browser" });
    expect(browser.getAttribute("data-deck-id")).toBe(deck.id);
    expect(browser.getAttribute("data-filters")).toBe("false");
    fireEvent.click(screen.getByRole("button", { name: "Practice" }));
    expect(practice).toHaveBeenCalledWith(deck.id);
    expect(screen.getByRole("heading", { name: deck.name })).toBeTruthy();
  });

  it.each([0, 4])("keeps direct Practice truthful for %s cards", (cardCount) => {
    const { practice } = mount({ ...deck, generatedCardCount: cardCount });
    const button = screen.getByRole("button", { name: "Practice" });
    expect(button.hasAttribute("disabled")).toBe(cardCount === 0);
    fireEvent.click(button);
    expect(practice).toHaveBeenCalledTimes(cardCount === 0 ? 0 : 1);
    expect(screen.getByRole("button", { name: `Delete ${deck.name}` }).getAttribute("data-tone")).toBe("danger");
  });

  it("disables archived Practice and restores the exact revision", async () => {
    const archived = { ...deck, archivedAt: 50, revision: 8 };
    const restored = { ...archived, archivedAt: null, revision: 9 };
    calls.setArchived.mockResolvedValue(restored);
    const { refresh, practice } = mount(archived);
    refresh.mockResolvedValue([restored]);
    const button = screen.getByRole("button", { name: "Practice" });
    expect(button.hasAttribute("disabled")).toBe(true);
    expect(button.getAttribute("title")).toBe("Unarchive this deck before practicing");
    fireEvent.click(button);
    expect(practice).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: `Restore ${deck.name}` }));
    await waitFor(() => expect(calls.setArchived).toHaveBeenCalledWith({ deckId: deck.id, revision: 8, archived: false }));
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
  });

  it("locks duplicate archive actions and preserves committed success on return failure", async () => {
    let complete!: (value: DeckSummary) => void;
    calls.setArchived.mockReturnValue(new Promise<DeckSummary>((resolve) => { complete = resolve; }));
    const { back, toast } = mount();
    back.mockRejectedValue(new Error("navigation unavailable"));
    const archive = screen.getByRole("button", { name: `Archive ${deck.name}` });
    fireEvent.click(archive); fireEvent.click(archive);
    expect(calls.setArchived).toHaveBeenCalledTimes(1);
    expect(calls.setArchived).toHaveBeenCalledWith({ deckId: deck.id, revision: 3, archived: true });
    await act(async () => { complete({ ...deck, archivedAt: 50, revision: 4 }); });
    await waitFor(() => expect(toast).toHaveBeenCalledWith("info", expect.stringContaining("navigation unavailable")));
    expect(toast.mock.calls.some(([kind]) => kind === "error")).toBe(false);
  });

  it("does not delete without confirmation", async () => {
    calls.confirm.mockResolvedValue(false);
    mount();
    fireEvent.click(screen.getByRole("button", { name: `Delete ${deck.name}` }));
    await waitFor(() => expect(calls.confirm).toHaveBeenCalledWith(expect.objectContaining({ confirmLabel: "Delete deck", destructive: true })));
    expect(calls.deleteDeck).not.toHaveBeenCalled();
  });

  it("publishes Undo after deletion and never invites a second restore after refresh failure", async () => {
    const { toast, refresh, back } = mount();
    back.mockRejectedValue(new Error("route unavailable"));
    fireEvent.click(screen.getByRole("button", { name: `Delete ${deck.name}` }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("success", `Deleted “${deck.name}”.`, expect.objectContaining({ label: "Undo" })));
    expect(calls.deleteDeck).toHaveBeenCalledWith(deck.id);
    const action = toast.mock.calls.find(([, , value]) => value?.label === "Undo")?.[2] as ToastAction;
    expect(action.expiresAt).toBeGreaterThan(Date.now());
    refresh.mockRejectedValueOnce(new Error("refresh unavailable"));
    await act(async () => { await action.run(); });
    expect(calls.undo).toHaveBeenCalledTimes(1);
    expect(calls.undo).toHaveBeenCalledWith("undo-token");
    expect(toast).toHaveBeenCalledWith("info", expect.stringContaining("restored"));
    expect(toast.mock.calls.some(([kind]) => kind === "error")).toBe(false);
  });

  it("opens the shared export workflow with the selected deck, not a separate exporter", () => {
    mount();
    const actions = screen.getByRole("group", { name: `${deck.name} actions` });
    fireEvent.click(within(actions).getByRole("button", { name: `Export ${deck.name}` }));
    expect(screen.getByRole("dialog", { name: "Export selected deck" }).textContent).toBe(deck.name);
  });
});

import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DeckSummary } from "../../ipc/decks";
import { DeckTransferDialog, type DeckTransferDialogRequest } from "./DeckTransferDialog";

const calls = vi.hoisted(() => ({
  importFromFile: vi.fn(), exportDecks: vi.fn(), shareDeck: vi.fn(), invalidate: vi.fn(),
}));
vi.mock("../../controllers/deckTransferController", () => ({
  DeckTransferController: class {
    importFromFile = calls.importFromFile; exportDecks = calls.exportDecks;
    shareDeck = calls.shareDeck; invalidate = calls.invalidate;
  },
}));
vi.mock("../../platform/PlatformProvider", () => ({ usePlatformCapabilities: () => ({ native: null }) }));
vi.mock("../../ui/overlays/Dialog", () => ({
  Dialog: ({ title, children, actions }: { title: string; children: ReactNode; actions?: ReactNode }) =>
    <section role="dialog" aria-label={title}>{children}<div data-testid="dialog-actions">{actions}</div></section>,
}));
const deck: DeckSummary = {
  id: "a".repeat(32), name: "Study examples", description: "", sourceNoteCount: 2,
  generatedCardCount: 4, studyableDueCount: 0, archivedAt: null,
  createdAt: 1, updatedAt: 1, revision: 3,
};
const outcome = { deckId: deck.id, committedDeck: deck, message: "Imported 2 entries; 1 exact duplicate was skipped." };
function mount(request: DeckTransferDialogRequest = { kind: "import" }) {
  const refresh = vi.fn().mockResolvedValue([deck]), open = vi.fn().mockResolvedValue(undefined);
  const close = vi.fn(), toast = vi.fn();
  render(<DeckTransferDialog request={request} refreshDecks={refresh} onImportedDeck={open} onClose={close} showToast={toast} />);
  return { refresh, open, close, toast };
}
beforeEach(() => { vi.resetAllMocks(); calls.importFromFile.mockResolvedValue(outcome); });

describe("active deck transfer presentation", () => {
  it("uses the shared responsive action owner and one explicit import action", () => {
    mount();
    const actions = screen.getByTestId("dialog-actions");
    expect(within(actions).getByRole("button", { name: "Import deck" })).toBeTruthy();
    expect(within(actions).queryByRole("button", { name: /Choose file/ })).toBeNull();
    expect(screen.queryByTestId("folder-import-decoration")).toBeNull();
  });
  it("leaves the library unchanged when the native file chooser is canceled", async () => {
    calls.importFromFile.mockResolvedValue(null);
    const { close, open, refresh, toast } = mount();
    const choose = screen.getByRole("button", { name: /Import deck/ });
    fireEvent.click(choose);
    await waitFor(() => expect(choose.hasAttribute("disabled")).toBe(false));
    expect(calls.importFromFile).toHaveBeenCalledOnce();
    expect(close).not.toHaveBeenCalled(); expect(open).not.toHaveBeenCalled();
    expect(refresh).not.toHaveBeenCalled(); expect(toast).not.toHaveBeenCalled();
  });
  it("shows a failed import in the same dialog and allows a deliberate retry", async () => {
    calls.importFromFile.mockRejectedValueOnce(new Error("Unsupported external file."));
    const { close, open } = mount();
    fireEvent.click(screen.getByRole("button", { name: /Import deck/ }));
    expect((await screen.findByRole("alert")).textContent).toContain("Unsupported external file");
    expect(close).not.toHaveBeenCalled(); expect(open).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /Import deck/ }));
    await waitFor(() => expect(open).toHaveBeenCalledWith(outcome));
    expect(calls.importFromFile).toHaveBeenCalledTimes(2);
  });
  it("reports the authoritative import/duplicate outcome and opens its committed identity", async () => {
    const { toast, open, refresh } = mount();
    fireEvent.click(screen.getByRole("button", { name: /Import deck/ }));
    await waitFor(() => expect(open).toHaveBeenCalledWith(outcome));
    expect(toast).toHaveBeenCalledWith("success", outcome.message);
    expect(refresh).toHaveBeenCalledOnce();
    expect(calls.importFromFile).toHaveBeenCalledOnce();
  });
  it("keeps committed non-Anki imports successful when refresh or navigation fails", async () => {
    const { toast, refresh, open } = mount();
    refresh.mockRejectedValueOnce(new Error("refresh failed"));
    open.mockRejectedValueOnce(new Error("route failed"));
    fireEvent.click(screen.getByRole("button", { name: /Import deck/ }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("info", expect.stringContaining("route failed")));
    expect(toast).toHaveBeenCalledWith("info", expect.stringContaining("refresh failed"));
    expect(toast.mock.calls.some(([kind]) => kind === "error")).toBe(false);
    expect(calls.importFromFile).toHaveBeenCalledOnce();
  });
  it("does not announce a successful export when the save picker is canceled", async () => {
    calls.exportDecks.mockResolvedValue({ cancelled: true, exported: [], requestedCount: 1 });
    const { close, toast } = mount({ kind: "export", decks: [deck] });
    fireEvent.click(screen.getByRole("button", { name: /Save file/ }));
    await waitFor(() => expect(close).toHaveBeenCalledOnce());
    expect(toast).not.toHaveBeenCalled();
    expect(calls.exportDecks).toHaveBeenCalledWith([deck]);
  });
  it("reports the exact authoritative total for a completed multi-deck export", async () => {
    const second = { ...deck, id: "b".repeat(32), name: "Second", generatedCardCount: 7 };
    calls.exportDecks.mockResolvedValue({
      cancelled: false, requestedCount: 2, exported: [
        { deck, cardCount: 4, receipt: { displayName: "first.studydeck" } },
        { deck: second, cardCount: 7, receipt: { displayName: "second.studydeck" } },
      ],
    });
    const { toast } = mount({ kind: "export", decks: [deck, second] });
    fireEvent.click(screen.getByRole("button", { name: /Save deck files/ }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("success", "Exported 2 deck files with 11 total cards."));
  });
  it("keeps the save action busy and prevents repeated publication", async () => {
    let finish!: (value: { cancelled: boolean; exported: []; requestedCount: number }) => void;
    calls.exportDecks.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    mount({ kind: "export", decks: [deck] });
    const save = screen.getByRole("button", { name: /Save file/ });
    fireEvent.click(save); fireEvent.click(save);
    expect(save.hasAttribute("disabled")).toBe(true);
    expect(calls.exportDecks).toHaveBeenCalledOnce();
    await act(async () => { finish({ cancelled: true, exported: [], requestedCount: 1 }); });
  });
  it("retains recoverable export errors without claiming a save or closing the dialog", async () => {
    calls.exportDecks.mockRejectedValueOnce(new Error("disk unavailable"));
    const { close, toast } = mount({ kind: "export", decks: [deck] });
    fireEvent.click(screen.getByRole("button", { name: /Save file/ }));
    expect((await screen.findByRole("alert")).textContent).toContain("disk unavailable");
    expect(close).not.toHaveBeenCalled(); expect(toast).not.toHaveBeenCalled();
  });
});

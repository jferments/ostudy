import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { deleteCards } from "../../ipc/cards";
import { dispatchPlatformBack } from "../../platform/backNavigation";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { DeleteCardsModal } from "./DeleteCardsModal";

vi.mock("../../ipc/cards", () => ({ deleteCards: vi.fn(), undoDeletion: vi.fn() }));

function Harness({ onClosed }: { onClosed: () => void }) {
  const [open, setOpen] = useState(false);
  return <OverlayProvider>
    <button type="button" onClick={() => setOpen(true)}>Open delete</button>
    {open ? <DeleteCardsModal
      cards={[{ cardId: "a".repeat(32), noteId: "b".repeat(32), deckId: "c".repeat(32) }]}
      onClose={() => { onClosed(); setOpen(false); }}
      onComplete={vi.fn()} showToast={vi.fn()}
    /> : null}
  </OverlayProvider>;
}

describe("card dialogs use the shared dismissal lifecycle", () => {
  beforeEach(() => { vi.clearAllMocks(); });

  it.each(["escape", "native-back"])("blocks %s during a mutation and restores opener focus when dismissal becomes safe", async (method) => {
    let fail: (error: Error) => void = () => { throw new Error("request was not initialized"); };
    const pending = new Promise<never>((_resolve, reject) => { fail = reject; });
    vi.mocked(deleteCards).mockReturnValue(pending);
    const onClosed = vi.fn();
    render(<Harness onClosed={onClosed} />);
    const opener = screen.getByRole("button", { name: "Open delete" });
    opener.focus();
    fireEvent.click(opener);
    const dialog = await screen.findByRole("dialog", { name: "Delete selected cards" });
    await waitFor(() => expect(dialog.contains(document.activeElement)).toBe(true));
    fireEvent.click(screen.getByRole("button", { name: "Delete 1 card" }));
    await waitFor(() => expect(deleteCards).toHaveBeenCalledOnce());
    expect((screen.getByRole("button", { name: "Cancel" }) as HTMLButtonElement).disabled).toBe(true);

    if (method === "escape") fireEvent.keyDown(dialog, { key: "Escape" });
    else act(() => { expect(dispatchPlatformBack()).toBe(true); });
    expect(onClosed).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "Delete selected cards" })).toBe(dialog);

    await act(async () => { fail(new Error("Save refused by native owner")); });
    expect(await screen.findByText("Save refused by native owner")).not.toBeNull();
    expect((screen.getByRole("button", { name: "Cancel" }) as HTMLButtonElement).disabled).toBe(false);
    if (method === "escape") fireEvent.keyDown(dialog, { key: "Escape" });
    else act(() => { expect(dispatchPlatformBack()).toBe(true); });
    await waitFor(() => expect(onClosed).toHaveBeenCalledOnce());
    expect(onClosed).toHaveBeenCalledWith();
    await waitFor(() => expect(document.activeElement).toBe(opener));
    expect(screen.queryByRole("dialog", { name: "Delete selected cards" })).toBeNull();
    expect(deleteCards).toHaveBeenCalledOnce();
  });
});

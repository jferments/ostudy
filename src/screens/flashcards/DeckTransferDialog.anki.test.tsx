import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { DeckTransferDialog } from "./DeckTransferDialog";

const mocks = vi.hoisted(() => ({ importFromFile: vi.fn(), invalidate: vi.fn() }));
vi.mock("../../controllers/deckTransferController", () => ({
  DeckTransferController: class {
    importFromFile = mocks.importFromFile;
    invalidate = mocks.invalidate;
  },
}));
vi.mock("../../platform/PlatformProvider", () => ({ usePlatformCapabilities: () => ({ native: null }) }));
vi.mock("../../ui/icons/Icon", () => ({ Icon: () => null }));
vi.mock("../../ui/overlays/Dialog", () => ({
  Dialog: ({ title, children }: { title: string; children: ReactNode }) => (
    <div role="dialog" aria-label={title}>{children}</div>
  ),
}));
vi.mock("../../ui/primitives/Button", () => ({
  Button: ({ children, onClick, disabled }: {
    children: ReactNode; onClick?: () => void; disabled?: boolean;
  }) => <button type="button" onClick={onClick} disabled={disabled}>{children}</button>,
}));
vi.mock("../../ui/primitives/InlineNotice", () => ({
  InlineNotice: ({ children }: { children: ReactNode }) => <div role="alert">{children}</div>,
}));

const outcome = {
  deckId: "a".repeat(32), committedDeck: null, message: "Imported Examples with 5 cards.",
  ankiReport: {
    sourceCardCount: 6, importedCardCount: 5, skippedCardCount: 1,
    simplifiedCardCount: 3, omittedIssueCount: 2,
    issues: [{ sourceCardId: "105", message: "Skipped: <script>not executable</script>" }],
  },
};

function mount(refresh = vi.fn().mockResolvedValue([]), open = vi.fn().mockResolvedValue(undefined)) {
  const close = vi.fn();
  const toast = vi.fn();
  render(<DeckTransferDialog request={{ kind: "import" }} refreshDecks={refresh}
    onImportedDeck={open} onClose={close} showToast={toast} />);
  return { close, open, toast, refresh };
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.importFromFile.mockResolvedValue(outcome);
});

describe("Anki import completion", () => {
  it("keeps the post-commit report visible until the user opens the imported deck", async () => {
    const { close, open } = mount();
    fireEvent.click(screen.getByRole("button", { name: /Choose file/ }));
    expect(await screen.findByRole("dialog", { name: "Anki import complete" })).not.toBeNull();
    expect(screen.getByLabelText("Anki import report").textContent).toContain("5 of 6 Anki cards imported");
    expect(screen.queryByRole("button", { name: /Choose file/ })).toBeNull();
    expect(close).not.toHaveBeenCalled();
    expect(open).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("Compatibility details (1)"));
    expect(screen.getByText(/not executable/).textContent).toContain("<script>not executable</script>");
    expect(document.querySelector("script")).toBeNull();
    expect(screen.getByText(/2 additional details omitted/)).not.toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open imported deck" }));
    await waitFor(() => expect(open).toHaveBeenCalledWith(outcome));
    expect(close).toHaveBeenCalledOnce();
    expect(mocks.importFromFile).toHaveBeenCalledOnce();
  });

  it("does not present a refresh failure as an import failure or permit a duplicate submit", async () => {
    const refresh = vi.fn().mockRejectedValue(new Error("refresh unavailable"));
    const { toast } = mount(refresh);
    fireEvent.click(screen.getByRole("button", { name: /Choose file/ }));
    await screen.findByRole("dialog", { name: "Anki import complete" });
    expect(screen.getByRole("status").textContent).toContain("refresh unavailable");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(toast).toHaveBeenCalledWith("info", expect.stringContaining("Imported Examples"));
    expect(mocks.importFromFile).toHaveBeenCalledOnce();
  });

  it("reports navigation failure as committed success rather than retryable import failure", async () => {
    const open = vi.fn().mockRejectedValue(new Error("navigation unavailable"));
    const { toast } = mount(undefined, open);
    fireEvent.click(screen.getByRole("button", { name: /Choose file/ }));
    await screen.findByRole("dialog", { name: "Anki import complete" });
    fireEvent.click(screen.getByRole("button", { name: "Open imported deck" }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("info", expect.stringContaining("The deck was imported but could not be opened")));
    expect(mocks.importFromFile).toHaveBeenCalledOnce();
  });

  it("leaves unsupported-package errors in the existing import dialog", async () => {
    mocks.importFromFile.mockRejectedValue(new Error("Export with Support older Anki versions enabled."));
    const { close } = mount();
    fireEvent.click(screen.getByRole("button", { name: /Choose file/ }));
    expect((await screen.findByRole("alert")).textContent).toContain("Support older Anki versions");
    expect(close).not.toHaveBeenCalled();
  });
});

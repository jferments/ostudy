import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../../ipc";
import { ScheduleActionModal } from "./ScheduleActionModal";

vi.mock("../../ipc", () => ({
  applyScheduleAction: vi.fn(),
  createMutationId: vi
    .fn()
    .mockReturnValueOnce("1".repeat(32))
    .mockReturnValueOnce("2".repeat(32))
    .mockReturnValue("3".repeat(32)),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));

const cards: ipc.SelectionCard[] = [
  { cardId: "a".repeat(32), noteId: "b".repeat(32), deckId: "c".repeat(32) },
  { cardId: "d".repeat(32), noteId: "e".repeat(32), deckId: "c".repeat(32) },
];

describe("ScheduleActionModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.applyScheduleAction).mockResolvedValue({
      affectedCount: 1,
      action: "reset",
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("applies reset to every exact selected card and preserves history messaging", async () => {
    const onComplete = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    const showToast = vi.fn();
    render(
      <ScheduleActionModal
        cards={cards}
        action="reset"
        onClose={onClose}
        onComplete={onComplete}
        showToast={showToast}
      />,
    );

    expect(screen.getByText(/Existing review history remains/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Reset schedule" }));

    await waitFor(() => {
      expect(ipc.applyScheduleAction).toHaveBeenCalledTimes(2);
      expect(ipc.applyScheduleAction).toHaveBeenNthCalledWith(
        1,
        expect.objectContaining({ cardIds: [cards[0]?.cardId], action: "reset" }),
      );
      expect(ipc.applyScheduleAction).toHaveBeenNthCalledWith(
        2,
        expect.objectContaining({ cardIds: [cards[1]?.cardId], action: "reset" }),
      );
      expect(onComplete).toHaveBeenCalledOnce();
      expect(showToast).toHaveBeenCalledWith("success", "Reset 2 cards to New.");
      expect(onClose).toHaveBeenCalledOnce();
    });
  });

  it("resolves a preset due date at local noon", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 7, 3, 8, 30, 0, 0));
    const onComplete = vi.fn().mockResolvedValue(undefined);
    render(
      <ScheduleActionModal
        cards={[cards[0] as ipc.SelectionCard]}
        action="reschedule"
        onClose={vi.fn()}
        onComplete={onComplete}
        showToast={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Reschedule" }));

    const expected = new Date(2026, 7, 4, 12, 0, 0, 0).getTime();
    await waitFor(() => {
      expect(ipc.applyScheduleAction).toHaveBeenCalledWith(
        expect.objectContaining({
          cardIds: [cards[0]?.cardId],
          action: "reschedule",
          dueAt: expected,
        }),
      );
    });
  });

  it("rejects an empty custom date without issuing a mutation", () => {
    render(
      <ScheduleActionModal
        cards={[cards[0] as ipc.SelectionCard]}
        action="reschedule"
        onClose={vi.fn()}
        onComplete={vi.fn().mockResolvedValue(undefined)}
        showToast={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByRole("combobox", { name: "New due date" }), {
      target: { value: "custom" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Reschedule" }));

    expect(screen.getByText("Choose a valid custom due date.")).toBeTruthy();
    expect(ipc.applyScheduleAction).not.toHaveBeenCalled();
  });

  it("surfaces a native failure and keeps the dialog open", async () => {
    vi.mocked(ipc.applyScheduleAction).mockRejectedValueOnce(
      new Error("Schedule changed in another window."),
    );
    const onClose = vi.fn();
    render(
      <ScheduleActionModal
        cards={[cards[0] as ipc.SelectionCard]}
        action="reset"
        onClose={onClose}
        onComplete={vi.fn().mockResolvedValue(undefined)}
        showToast={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Reset schedule" }));

    expect(
      await screen.findByText("Schedule changed in another window."),
    ).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });
});

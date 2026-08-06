import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../../ipc";
import { ScheduleActionModal } from "./ScheduleActionModal";

vi.mock("../../ipc", () => ({
  applyScheduleAction: vi.fn(),
  createMutationId: vi.fn(() => "1".repeat(32)),
  getErrorCode: (error: unknown) => {
    if (typeof error !== "object" || error === null) return null;
    const code = (error as { code?: unknown }).code;
    return typeof code === "string" ? code : null;
  },
  getErrorMessage: (error: unknown) => {
    if (error instanceof Error) return error.message;
    if (typeof error === "object" && error !== null) {
      const message = (error as { message?: unknown }).message;
      if (typeof message === "string") return message;
    }
    return "Unexpected error";
  },
}));

const cards: ipc.SelectionCard[] = [
  { cardId: "a".repeat(32), noteId: "b".repeat(32), deckId: "c".repeat(32) },
  { cardId: "d".repeat(32), noteId: "e".repeat(32), deckId: "c".repeat(32) },
];

function result(
  action: ipc.ScheduleActionKind,
  changedCount = 2,
): ipc.ScheduleActionResult {
  return {
    affectedCount: changedCount,
    requestedCount: 2,
    changedCount,
    unchangedCount: 2 - changedCount,
    rejectedCount: 0,
    action,
    resolvedDueAt: action === "reschedule" ? Date.now() + 86_400_000 : null,
  };
}

describe("ScheduleActionModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.applyScheduleAction).mockResolvedValue(result("reset"));
  });

  it("applies the complete reset selection in one transaction", async () => {
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
    expect(screen.getByText(/one transaction or not at all/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Reset schedule" }));

    await waitFor(() => {
      expect(ipc.applyScheduleAction).toHaveBeenCalledOnce();
      expect(ipc.applyScheduleAction).toHaveBeenCalledWith({
        mutationId: "1".repeat(32),
        cardIds: cards.map((card) => card.cardId).sort(),
        action: "reset",
        rescheduleIntent: undefined,
      });
      expect(onComplete).toHaveBeenCalledOnce();
      expect(showToast).toHaveBeenCalledWith("success", "Reset 2 cards to New.");
      expect(onClose).toHaveBeenCalledOnce();
    });
  });

  it("reports changed and already-satisfied cards from one batch result", async () => {
    vi.mocked(ipc.applyScheduleAction).mockResolvedValue(result("reset", 1));
    const showToast = vi.fn();
    render(
      <ScheduleActionModal
        cards={cards}
        action="reset"
        onClose={vi.fn()}
        onComplete={vi.fn().mockResolvedValue(undefined)}
        showToast={showToast}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Reset schedule" }));

    await waitFor(() => {
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Reset 1 card to New. 1 card was already New and due.",
      );
    });
  });

  it("submits tomorrow as an authoritative backend calendar intent", async () => {
    vi.mocked(ipc.applyScheduleAction).mockResolvedValue({
      ...result("reschedule", 1),
      requestedCount: 1,
      unchangedCount: 0,
    });
    render(
      <ScheduleActionModal
        cards={[cards[0] as ipc.SelectionCard]}
        action="reschedule"
        onClose={vi.fn()}
        onComplete={vi.fn().mockResolvedValue(undefined)}
        showToast={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Reschedule" }));

    await waitFor(() => {
      expect(ipc.applyScheduleAction).toHaveBeenCalledWith({
        mutationId: "1".repeat(32),
        cardIds: [cards[0]?.cardId],
        action: "reschedule",
        rescheduleIntent: { kind: "tomorrow" },
      });
    });
  });

  it("submits a custom date without constructing a browser timestamp", async () => {
    vi.mocked(ipc.applyScheduleAction).mockResolvedValue({
      ...result("reschedule", 1),
      requestedCount: 1,
      unchangedCount: 0,
    });
    render(
      <ScheduleActionModal
        cards={[cards[0] as ipc.SelectionCard]}
        action="reschedule"
        onClose={vi.fn()}
        onComplete={vi.fn().mockResolvedValue(undefined)}
        showToast={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByRole("combobox", { name: "New schedule date" }), {
      target: { value: "custom" },
    });
    fireEvent.change(screen.getByLabelText("Custom local date"), {
      target: { value: "2026-08-12" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Reschedule" }));

    await waitFor(() => {
      expect(ipc.applyScheduleAction).toHaveBeenCalledWith(
        expect.objectContaining({
          action: "reschedule",
          rescheduleIntent: { kind: "calendar_date", date: "2026-08-12" },
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

    fireEvent.change(screen.getByRole("combobox", { name: "New schedule date" }), {
      target: { value: "custom" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Reschedule" }));

    expect(screen.getByText("Choose a valid custom due date.")).toBeTruthy();
    expect(ipc.applyScheduleAction).not.toHaveBeenCalled();
  });

  it("reuses one mutation ID after an ambiguous batch response", async () => {
    vi.mocked(ipc.applyScheduleAction)
      .mockRejectedValueOnce(new Error("Native response was lost."))
      .mockResolvedValueOnce(result("reset"));
    const showToast = vi.fn();
    render(
      <ScheduleActionModal
        cards={cards}
        action="reset"
        onClose={vi.fn()}
        onComplete={vi.fn().mockResolvedValue(undefined)}
        showToast={showToast}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Reset schedule" }));
    expect(await screen.findByText(/safely retry the complete selection/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Reset schedule" }));

    await waitFor(() => {
      expect(ipc.applyScheduleAction).toHaveBeenCalledTimes(2);
      expect(ipc.createMutationId).toHaveBeenCalledOnce();
      expect(vi.mocked(ipc.applyScheduleAction).mock.calls[1]?.[0]).toEqual(
        vi.mocked(ipc.applyScheduleAction).mock.calls[0]?.[0],
      );
      expect(showToast).toHaveBeenCalledWith("success", "Reset 2 cards to New.");
    });
  });

  it("surfaces an authoritative native failure and keeps the dialog open", async () => {
    vi.mocked(ipc.applyScheduleAction).mockRejectedValueOnce({
      code: "schedule_card_not_found",
      message: "One selected card disappeared.",
    });
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

    expect(await screen.findByText("One selected card disappeared.")).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });
});

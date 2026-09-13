import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { Toast } from "./Toast";

afterEach(() => {
  vi.useRealTimers();
});

describe("Toast", () => {
  it("renders a status message and dismisses it", () => {
    const onDismiss = vi.fn();

    render(
      <Toast
        toast={{ id: 1, kind: "success", text: "Deck saved." }}
        onDismiss={onDismiss}
      />,
    );

    expect(screen.getByRole("status").textContent).toContain("Deck saved.");
    fireEvent.click(screen.getByRole("button", { name: "Dismiss notification" }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("uses an alert role for errors", () => {
    render(
      <Toast
        toast={{ id: 2, kind: "error", text: "Could not save." }}
        onDismiss={() => undefined}
      />,
    );

    expect(screen.getByRole("alert").textContent).toContain("Could not save.");
  });

  it("pauses automatic dismissal while the user is interacting with an ordinary toast", () => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
    const onDismiss = vi.fn();
    render(
      <Toast
        toast={{ id: 5, kind: "info", text: "Import completed." }}
        onDismiss={onDismiss}
      />,
    );

    const toast = screen.getByRole("status");
    fireEvent.pointerEnter(toast, { pointerType: "mouse" });
    fireEvent.mouseEnter(toast);
    vi.advanceTimersByTime(10_000);
    expect(onDismiss).not.toHaveBeenCalled();

    fireEvent.pointerLeave(toast, { pointerType: "mouse" });
    fireEvent.mouseLeave(toast);
    vi.runOnlyPendingTimers();
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("keeps Undo actionable for the whole toast lifetime and dismisses the toast instead of showing an expired state", () => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
    const onDismiss = vi.fn();
    render(
      <Toast
        toast={{
          id: 6,
          kind: "success",
          text: "Deck deleted.",
          action: { label: "Undo", run: vi.fn(), expiresAt: 60_000 },
        }}
        onDismiss={onDismiss}
      />,
    );

    const toast = screen.getByRole("status");
    const undo = screen.getByRole("button", { name: "Undo" });
    expect(undo.className).toContain("tw:bg-primary");
    fireEvent.pointerEnter(toast, { pointerType: "mouse" });
    fireEvent.mouseEnter(toast);

    vi.advanceTimersByTime(9_999);
    expect(onDismiss).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Undo" })).toBeTruthy();
    expect(screen.queryByText(/Undo expired/u)).toBeNull();

    vi.advanceTimersByTime(1);
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("runs an undo action once, shows progress, and dismisses after success", async () => {
    let resolveUndo!: () => void;
    const run = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveUndo = resolve;
        }),
    );
    const onDismiss = vi.fn();
    render(
      <Toast
        toast={{
          id: 3,
          kind: "success",
          text: "Flashcard deleted.",
          action: { label: "Undo", run, expiresAt: Date.now() + 10_000 },
        }}
        onDismiss={onDismiss}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    expect(run).toHaveBeenCalledOnce();
    expect(
      (screen.getByRole("button", { name: "Restoring…" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);

    resolveUndo();
    await waitFor(() => expect(onDismiss).toHaveBeenCalledOnce());
  });

  it("reenables an undo action when restoration fails before the toast deadline", async () => {
    const run = vi.fn().mockRejectedValue(new Error("Temporary failure"));
    render(
      <Toast
        toast={{
          id: 4,
          kind: "success",
          text: "Deck deleted.",
          action: { label: "Undo", run, expiresAt: Date.now() + 10_000 },
        }}
        onDismiss={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => {
      expect(
        (screen.getByRole("button", { name: "Undo" }) as HTMLButtonElement).disabled,
      ).toBe(false);
    });
  });
});

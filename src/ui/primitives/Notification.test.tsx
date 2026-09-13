import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Notification } from "./Notification";
afterEach(() => vi.useRealTimers());

describe("Notification action lifetime", () => {
  it("survives StrictMode effect replay without a duplicate or premature dismissal", () => {
    const dismiss = vi.fn();
    render(<StrictMode><Notification message={{ id: 8, kind: "info", text: "Ready" }} onDismiss={dismiss} /></StrictMode>);
    expect(screen.getAllByRole("status")).toHaveLength(1);
    expect(screen.getByRole("status").textContent).toContain("Ready");
    expect(dismiss).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss notification" }));
    expect(dismiss).toHaveBeenCalledOnce();
  });
  it("dismisses the whole notification at the action deadline instead of rendering an expired action", async () => {
    vi.useFakeTimers(); vi.setSystemTime(0);
    const run = vi.fn(); const onDismiss = vi.fn();
    render(<Notification message={{ id: 1, kind: "success", text: "Deleted", action: { label: "Undo", run, expiresAt: 1000 } }} onDismiss={onDismiss} />);
    fireEvent.mouseEnter(screen.getByRole("status"));
    await act(async () => { await vi.advanceTimersByTimeAsync(999); });
    expect(screen.getByRole("button", { name: "Undo" })).toBeTruthy();
    expect(onDismiss).not.toHaveBeenCalled();
    await act(async () => { await vi.advanceTimersByTimeAsync(1); });
    expect(onDismiss).toHaveBeenCalledOnce();
    expect(screen.queryByRole("button", { name: "Undo expired" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    expect(run).not.toHaveBeenCalled();
  });
  it("does not dismiss a newer notification when an older async action completes", async () => {
    let resolve!: () => void;
    const firstDismiss = vi.fn(); const secondDismiss = vi.fn();
    const run = vi.fn(() => new Promise<void>((done) => { resolve = done; }));
    const view = render(<Notification key="1" message={{ id: 1, kind: "success", text: "First", action: { label: "Undo", run } }} onDismiss={firstDismiss} />);
    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    view.rerender(<Notification key="2" message={{ id: 2, kind: "info", text: "Second" }} onDismiss={secondDismiss} />);
    await act(async () => { resolve(); });
    expect(firstDismiss).not.toHaveBeenCalled(); expect(secondDismiss).not.toHaveBeenCalled();
    expect(screen.getByRole("status").textContent).toContain("Second");
  });
  it("does not run the same action twice while its first request is unresolved", async () => {
    let resolve!: () => void;
    const run = vi.fn(() => new Promise<void>((done) => { resolve = done; })); const onDismiss = vi.fn();
    render(<Notification message={{ id: 3, kind: "success", text: "Deleted", action: { label: "Undo", run } }} onDismiss={onDismiss} />);
    const button = screen.getByRole("button", { name: "Undo" });
    fireEvent.click(button); fireEvent.click(button); expect(run).toHaveBeenCalledOnce();
    await act(async () => { resolve(); });
    await waitFor(() => expect(onDismiss).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByRole("button", { name: "Dismiss notification" }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });
  it.each(["light", "dark", "console", "synthwave"])("keeps one accessible error announcement in %s", (theme) => {
    document.documentElement.dataset.theme = theme;
    try {
      render(<Notification message={{ id: 4, kind: "error", text: "Please retry." }} onDismiss={vi.fn()} />);
      expect(screen.getAllByRole("alert")).toHaveLength(1);
      expect(screen.getByRole("alert").textContent).toContain("Please retry.");
    } finally { delete document.documentElement.dataset.theme; }
  });
});

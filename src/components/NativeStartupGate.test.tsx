import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { getSettings } from "../ipc/settings";
import { observeLibraryUpgrade, type LibraryUpgradeProgress } from "../ipc/libraryUpgrade";
import { NativeStartupGate, waitForNativeStartup } from "./NativeStartupGate";

vi.mock("../ipc/settings", () => ({
  getSettings: vi.fn(),
}));
vi.mock("../ipc/libraryUpgrade", () => ({ observeLibraryUpgrade: vi.fn() }));

beforeEach(() => {
  vi.mocked(observeLibraryUpgrade).mockResolvedValue(() => {});
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("NativeStartupGate", () => {
  it("waits until the database-backed settings command is usable before mounting application children", async () => {
    vi.mocked(getSettings)
      .mockRejectedValueOnce(new Error("database not managed yet"))
      .mockResolvedValue({} as Awaited<ReturnType<typeof getSettings>>);

    render(<NativeStartupGate><div>Application ready</div></NativeStartupGate>);

    expect(screen.getByText("Opening ostudy")).toBeTruthy();
    expect(await screen.findByText("Application ready")).toBeTruthy();
    expect(getSettings).toHaveBeenCalledTimes(2);
  });

  it("resolves immediately once native managed state is published", async () => {
    vi.mocked(getSettings).mockResolvedValue(
      {} as Awaited<ReturnType<typeof getSettings>>,
    );
    await expect(waitForNativeStartup(100)).resolves.toBeUndefined();
    await waitFor(() => expect(getSettings).toHaveBeenCalledOnce());
  });

  it("keeps an explicit bounded probe available for diagnostics without making the product splash time out", async () => {
    vi.mocked(getSettings).mockRejectedValue(new Error("database not managed yet"));
    await expect(waitForNativeStartup(30, 5)).rejects.toThrow(/did not become ready/iu);
  });

  it("retries when one bridge invocation itself hangs instead of freezing the splash forever", async () => {
    vi.mocked(getSettings)
      .mockImplementationOnce(() => new Promise<never>(() => undefined))
      .mockResolvedValue({} as Awaited<ReturnType<typeof getSettings>>);

    await expect(waitForNativeStartup(250, 25)).resolves.toBeUndefined();
    expect(getSettings).toHaveBeenCalledTimes(2);
  });

  it("shows upgrade phases but does not admit children on a completion event alone", async () => {
    let finish: ((value: Awaited<ReturnType<typeof getSettings>>) => void) | undefined;
    vi.mocked(getSettings).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    let progress: ((value: LibraryUpgradeProgress) => void) | undefined;
    const unsubscribe = vi.fn();
    vi.mocked(observeLibraryUpgrade).mockImplementation(async (changed) => {
      progress = changed;
      return unsubscribe;
    });
    const view = render(<NativeStartupGate><div>Application ready</div></NativeStartupGate>);
    act(() => progress?.({ sequence: 1, message: "Creating a recovery checkpoint…", completed: false }));
    expect(screen.getByRole("heading", { name: "Updating your oStudy library" })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Creating a recovery checkpoint");
    expect(screen.queryByRole("progressbar")).toBeNull();
    act(() => progress?.({ sequence: 2, message: "Creating a recovery checkpoint…", completed: false, permille: 425 }));
    expect(screen.getByRole("progressbar", { name: "Library update progress" }).getAttribute("aria-valuetext")).toBe("42%");
    expect(screen.getByText("42% done")).toBeTruthy();
    act(() => progress?.({ sequence: 3, message: "Library updated; recovery checkpoint retained.", completed: true }));
    expect(screen.queryByText("Application ready")).toBeNull();
    await act(async () => { finish?.({} as Awaited<ReturnType<typeof getSettings>>); });
    expect(await screen.findByText("Application ready")).toBeTruthy();
    expect(screen.getByText("Library updated; recovery checkpoint retained.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText("Library updated; recovery checkpoint retained.")).toBeNull();
    view.unmount();
    expect(unsubscribe).toHaveBeenCalledOnce();
  });

  it("unsubscribes a late registration after unmount without holding startup hostage", async () => {
    let registered: ((cleanup: () => void) => void) | undefined;
    vi.mocked(observeLibraryUpgrade).mockReturnValue(new Promise((resolve) => { registered = resolve; }));
    vi.mocked(getSettings).mockResolvedValue({} as Awaited<ReturnType<typeof getSettings>>);
    const view = render(<NativeStartupGate><div>Application ready</div></NativeStartupGate>);
    expect(await screen.findByText("Application ready")).toBeTruthy();
    view.unmount();
    const unsubscribe = vi.fn();
    await act(async () => { registered?.(unsubscribe); });
    expect(unsubscribe).toHaveBeenCalledOnce();
  });
});

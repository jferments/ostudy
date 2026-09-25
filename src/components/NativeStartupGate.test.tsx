import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { getSettings } from "../ipc/settings";
import { NativeStartupGate, waitForNativeStartup } from "./NativeStartupGate";

vi.mock("../ipc/settings", () => ({
  getSettings: vi.fn(),
}));

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
});

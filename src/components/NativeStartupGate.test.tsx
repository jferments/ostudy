import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { getNativePlatformSnapshot } from "../ipc/platform";
import { NativeStartupGate, waitForNativeStartup } from "./NativeStartupGate";

vi.mock("../ipc/platform", () => ({
  getNativePlatformSnapshot: vi.fn(),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("NativeStartupGate", () => {
  it("retries the cold-start state-publication race before mounting application children", async () => {
    vi.mocked(getNativePlatformSnapshot)
      .mockRejectedValueOnce(new Error("state not managed"))
      .mockResolvedValue({} as Awaited<ReturnType<typeof getNativePlatformSnapshot>>);

    render(<NativeStartupGate><div>Application ready</div></NativeStartupGate>);

    expect(screen.getByText("Opening ostudy")).toBeTruthy();
    expect(await screen.findByText("Application ready")).toBeTruthy();
    expect(getNativePlatformSnapshot).toHaveBeenCalledTimes(2);
  });

  it("resolves immediately once managed native state is available", async () => {
    vi.mocked(getNativePlatformSnapshot).mockResolvedValue(
      {} as Awaited<ReturnType<typeof getNativePlatformSnapshot>>,
    );
    await expect(waitForNativeStartup(100)).resolves.toBeUndefined();
    await waitFor(() => expect(getNativePlatformSnapshot).toHaveBeenCalledOnce());
  });
});

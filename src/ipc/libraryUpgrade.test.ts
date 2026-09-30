import { beforeEach, describe, expect, it, vi } from "vitest";
import { isTauri } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { observeLibraryUpgrade } from "./libraryUpgrade";

vi.mock("@tauri-apps/api/core", () => ({ isTauri: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ emit: vi.fn(), listen: vi.fn() }));

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(emit).mockResolvedValue();
});

describe("library upgrade progress", () => {
  it("subscribes before replay and ignores malformed, stale, and post-disposal events", async () => {
    const cleanup = vi.fn();
    vi.mocked(listen).mockResolvedValue(cleanup);
    vi.mocked(emit).mockImplementation(async () => { expect(listen).toHaveBeenCalledOnce(); });
    const changed = vi.fn();
    const unsubscribe = await observeLibraryUpgrade(changed);
    const callback = vi.mocked(listen).mock.calls[0]?.[1];
    expect(callback).toBeTypeOf("function");
    const deliver = (payload: unknown) => callback?.({ event: "ostudy-library-upgrade-progress", id: 1, payload });
    expect(emit).toHaveBeenCalledWith("ostudy-library-upgrade-progress-request");
    const current = { sequence: 2, message: "Verifying the updated library…", completed: false };
    deliver(current);
    deliver({ sequence: 1, message: "older phase", completed: false });
    deliver({ sequence: 3, message: 14, completed: false });
    deliver({ sequence: 3, message: "x".repeat(513), completed: false });
    deliver({ sequence: 2, message: "duplicate phase", completed: false });
    expect(changed).toHaveBeenCalledTimes(1);
    expect(changed).toHaveBeenLastCalledWith(current);
    unsubscribe();
    deliver({ sequence: 4, message: "late phase", completed: true });
    expect(changed).toHaveBeenCalledTimes(1);
    expect(cleanup).toHaveBeenCalledOnce();
  });

  it("releases listeners even when the best-effort snapshot request fails", async () => {
    const cleanup = vi.fn();
    vi.mocked(listen).mockResolvedValue(cleanup);
    vi.mocked(emit).mockRejectedValue(new Error("bridge unavailable"));
    const unsubscribe = await observeLibraryUpgrade(vi.fn());
    unsubscribe();
    expect(cleanup).toHaveBeenCalledOnce();
  });

  it("does not require native events in a browser preview", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    (await observeLibraryUpgrade(vi.fn()))();
    expect(listen).not.toHaveBeenCalled();
    expect(emit).not.toHaveBeenCalled();
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";
import { listenToAndroidBack, listenToLifecycle } from "./platform";

const mocks = vi.hoisted(() => ({ plugin: vi.fn(), listen: vi.fn(), report: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ addPluginListener: mocks.plugin, invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
vi.mock("./diagnostics", () => ({ reportFrontendFailure: mocks.report }));

beforeEach(() => { vi.resetAllMocks(); });

describe("native listener disposal", () => {
  it("contains asynchronous unregister rejection and unregisters only once", async () => {
    const failure = new Error("unregister unavailable");
    const unregister = vi.fn().mockRejectedValue(failure);
    mocks.plugin.mockResolvedValue({ unregister });
    const dispose = await listenToAndroidBack(vi.fn());
    dispose();
    dispose();
    await Promise.resolve();
    expect(unregister).toHaveBeenCalledOnce();
    expect(mocks.report).toHaveBeenCalledWith("platform_listener_cleanup_failed", failure);
  });

  it("contains a synchronous unregister failure", async () => {
    const failure = new Error("unregister threw");
    mocks.plugin.mockResolvedValue({ unregister: () => { throw failure; } });
    const dispose = await listenToAndroidBack(vi.fn());
    expect(dispose).not.toThrow();
    expect(mocks.report).toHaveBeenCalledWith("platform_listener_cleanup_failed", failure);
  });

  it("still unregisters the Android lifecycle listener if the shared listener disposal fails", async () => {
    const failure = new Error("core listener failed");
    const unregister = vi.fn().mockResolvedValue(undefined);
    mocks.plugin.mockResolvedValue({ unregister });
    mocks.listen.mockResolvedValue(() => { throw failure; });
    const dispose = await listenToLifecycle(vi.fn());
    expect(dispose).toThrow(failure);
    expect(unregister).toHaveBeenCalledOnce();
  });
});

import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  back: vi.fn(), insets: vi.fn(), network: vi.fn(), intercept: vi.fn(),
  initialize: vi.fn(), subscribe: vi.fn(), unsubscribe: vi.fn(),
  updateInsets: vi.fn(), updateNetwork: vi.fn(), dispatchBack: vi.fn(),
  subscribeBack: vi.fn(), report: vi.fn(),
}));
vi.mock("../ipc/diagnostics", () => ({ reportFrontendFailure: mocks.report }));
vi.mock("../ipc/platform", () => ({
  listenToAndroidBack: mocks.back, listenToAndroidInsets: mocks.insets,
  listenToAndroidNetwork: mocks.network, setPlatformBackInterception: mocks.intercept,
}));
vi.mock("./capabilities", () => ({
  getPlatformCapabilities: () => ({ native: { operatingSystem: "android" } }),
  initializePlatformCapabilities: mocks.initialize,
  subscribePlatformCapabilities: mocks.subscribe,
  updateAndroidInsets: mocks.updateInsets, updateAndroidNetwork: mocks.updateNetwork,
}));
vi.mock("./backNavigation", () => ({
  dispatchPlatformBack: mocks.dispatchBack, subscribePlatformBackStack: mocks.subscribeBack,
}));

import { PlatformProvider } from "./PlatformProvider";

function deferred<T>() {
  let complete: ((value: T) => void) | undefined;
  const promise = new Promise<T>((resolve) => { complete = resolve; });
  return { promise, resolve(value: T) {
    if (!complete) throw new Error("Deferred resolver missing");
    complete(value);
  } };
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.back.mockResolvedValue(vi.fn());
  mocks.insets.mockResolvedValue(vi.fn());
  mocks.network.mockResolvedValue(vi.fn());
  mocks.intercept.mockResolvedValue(undefined);
  mocks.initialize.mockResolvedValue(undefined);
  mocks.subscribe.mockReturnValue(mocks.unsubscribe);
  mocks.subscribeBack.mockReturnValue(vi.fn());
  mocks.dispatchBack.mockReturnValue(true);
  document.documentElement.style.removeProperty("--android-safe-top");
});

describe("platform listener lifetime and failure evidence", () => {
  it("records initialization and listener failures through named diagnostics", async () => {
    const error = new Error("native unavailable");
    mocks.initialize.mockRejectedValue(error);
    mocks.back.mockRejectedValue(error);
    mocks.insets.mockRejectedValue(error);
    mocks.network.mockRejectedValue(error);
    const view = render(<PlatformProvider><span>Study still works</span></PlatformProvider>);
    await waitFor(() => expect(mocks.report).toHaveBeenCalledTimes(4));
    for (const event of ["platform_initialization_failed", "android_back_listener_failed", "android_insets_listener_failed", "android_network_listener_failed"]) {
      expect(mocks.report).toHaveBeenCalledWith(event, error);
    }
    expect(view.getByText("Study still works")).toBeTruthy();
    view.unmount();
  });

  it("disposes late registrations and ignores late native callbacks after unmount", async () => {
    const back = deferred<() => void>();
    const insets = deferred<() => void>();
    const network = deferred<() => void>();
    mocks.back.mockReturnValue(back.promise);
    mocks.insets.mockReturnValue(insets.promise);
    mocks.network.mockReturnValue(network.promise);
    const view = render(<PlatformProvider><span>Study</span></PlatformProvider>);
    expect(mocks.back).toHaveBeenCalledOnce();
    expect(mocks.insets).toHaveBeenCalledOnce();
    expect(mocks.network).toHaveBeenCalledOnce();
    view.unmount();
    const removeBack = vi.fn();
    const removeInsets = vi.fn();
    const removeNetwork = vi.fn();
    await act(async () => {
      mocks.back.mock.calls[0]?.[0]();
      mocks.insets.mock.calls[0]?.[0]({ top: 99, right: 0, bottom: 0, left: 0, imeBottom: 0, imeVisible: false });
      mocks.network.mock.calls[0]?.[0]({ online: true, metered: false, validatedInternet: true, transport: "wifi" });
      back.resolve(removeBack);
      insets.resolve(removeInsets);
      network.resolve(removeNetwork);
    });
    expect(removeBack).toHaveBeenCalledOnce();
    expect(removeInsets).toHaveBeenCalledOnce();
    expect(removeNetwork).toHaveBeenCalledOnce();
    expect(mocks.unsubscribe).toHaveBeenCalledOnce();
    expect(mocks.dispatchBack).not.toHaveBeenCalled();
    expect(mocks.updateInsets).not.toHaveBeenCalled();
    expect(mocks.updateNetwork).not.toHaveBeenCalled();
    expect(mocks.subscribeBack).not.toHaveBeenCalled();
    expect(document.documentElement.style.getPropertyValue("--android-safe-top")).toBe("");
    expect(document.documentElement.dataset.androidBackListener).toBeUndefined();
    expect(document.documentElement.dataset.androidBackInterception).toBeUndefined();
  });

  it("continues listener cleanup when one disposer fails", async () => {
    const error = new Error("unregister failed");
    mocks.back.mockResolvedValue(() => { throw error; });
    const removeInsets = vi.fn();
    const removeNetwork = vi.fn();
    mocks.insets.mockResolvedValue(removeInsets);
    mocks.network.mockResolvedValue(removeNetwork);
    const view = render(<PlatformProvider><span>Study</span></PlatformProvider>);
    await waitFor(() => expect(document.documentElement.dataset.androidBackListener).toBe("ready"));
    view.unmount();
    expect(mocks.report).toHaveBeenCalledWith("platform_listener_cleanup_failed", error);
    expect(removeInsets).toHaveBeenCalledOnce();
    expect(removeNetwork).toHaveBeenCalledOnce();
  });
});

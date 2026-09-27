import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ImagePasteSession } from "./imagePaste";

const mocks = vi.hoisted(() => ({ read: vi.fn(), report: vi.fn() }));
vi.mock("../ipc/diagnostics", () => ({ reportFrontendFailure: mocks.report }));
vi.mock("./imageInput", async (original) => ({
  ...await original<typeof import("./imageInput")>(), readClipboardImageFiles: mocks.read,
}));

const disposers: Array<() => void> = [];
function image() { return new File(["pixels"], "screenshot.png", { type: "image/png" }); }
function shortcut() { window.dispatchEvent(new KeyboardEvent("keydown", { key: "v", ctrlKey: true })); }
function paste(files: File[]) {
  const event = new Event("paste", { cancelable: true });
  Object.defineProperty(event, "clipboardData", { value: { files, items: [] } });
  window.dispatchEvent(event);
  return event;
}
function deferred<T>() {
  let complete: ((value: T) => void) | undefined;
  const promise = new Promise<T>((resolve) => { complete = resolve; });
  return { promise, resolve(value: T) {
    if (!complete) throw new Error("Missing resolver");
    complete(value);
  } };
}
function bind(accept: (files: File[]) => void, session = new ImagePasteSession()) {
  const dispose = session.bind(accept);
  disposers.push(dispose);
  return { session, dispose };
}

beforeEach(() => { vi.useFakeTimers(); mocks.read.mockReset().mockResolvedValue([]); mocks.report.mockReset(); });
afterEach(() => { for (const dispose of disposers.splice(0)) dispose(); vi.useRealTimers(); });

describe("image paste gesture and owner lifetime", () => {
  it("lets the ordinary native image paste win before the fallback timer", async () => {
    const accept = vi.fn();
    bind(accept);
    shortcut();
    expect(paste([image()]).defaultPrevented).toBe(true);
    await vi.runAllTimersAsync();
    expect(accept).toHaveBeenCalledOnce();
    expect(mocks.read).not.toHaveBeenCalled();
  });

  it("ignores a pending read after native paste claims the gesture", async () => {
    const read = deferred<File[]>();
    mocks.read.mockReturnValue(read.promise);
    const accept = vi.fn();
    bind(accept);
    shortcut();
    await vi.runAllTimersAsync();
    expect(mocks.read).toHaveBeenCalledOnce();
    paste([image()]);
    read.resolve([image()]);
    await read.promise;
    expect(accept).toHaveBeenCalledOnce();
  });

  it("does not add a late native event again after the fallback already completed", async () => {
    mocks.read.mockResolvedValue([image()]);
    const accept = vi.fn();
    const { session, dispose } = bind(accept);
    shortcut();
    await vi.runAllTimersAsync();
    expect(accept).toHaveBeenCalledOnce();
    dispose();
    const refreshedSource = vi.fn();
    bind(refreshedSource, session);
    expect(paste([image()]).defaultPrevented).toBe(true);
    expect(refreshedSource).not.toHaveBeenCalled();
    shortcut();
    await vi.runAllTimersAsync();
    expect(refreshedSource).toHaveBeenCalledOnce();
  });

  it("cancels the fallback timer when its acquisition owner leaves", async () => {
    const accept = vi.fn();
    const { dispose } = bind(accept);
    shortcut();
    dispose();
    await vi.runAllTimersAsync();
    expect(mocks.read).not.toHaveBeenCalled();
    expect(accept).not.toHaveBeenCalled();
  });

  it("rejects a late read after unmount or source rebinding without cancelling accepted jobs", async () => {
    const read = deferred<File[]>();
    mocks.read.mockReturnValueOnce(read.promise);
    const oldSource = vi.fn();
    const { session, dispose } = bind(oldSource);
    shortcut();
    await vi.runAllTimersAsync();
    dispose();
    const newSource = vi.fn();
    bind(newSource, session);
    read.resolve([image()]);
    await read.promise;
    expect(oldSource).not.toHaveBeenCalled();
    expect(newSource).not.toHaveBeenCalled();
    paste([image()]);
    expect(newSource).toHaveBeenCalledOnce();
  });

  it("allows a later context-menu paste and leaves text-only paste untouched", async () => {
    mocks.read.mockResolvedValue([image()]);
    const accept = vi.fn();
    bind(accept);
    shortcut();
    await vi.runAllTimersAsync();
    window.dispatchEvent(new Event("contextmenu"));
    paste([image()]);
    expect(accept).toHaveBeenCalledTimes(2);
    expect(paste([]).defaultPrevented).toBe(false);
  });

  it("handles unavailable clipboard permission without logging captured clipboard values", async () => {
    const denied = new DOMException("private clipboard metadata", "NotAllowedError");
    mocks.read.mockRejectedValue(denied);
    const accept = vi.fn();
    bind(accept);
    shortcut();
    await vi.runAllTimersAsync();
    expect(mocks.report).toHaveBeenCalledWith("clipboard_image_read_failed", denied);
    expect(accept).not.toHaveBeenCalled();
    paste([image()]);
    expect(accept).toHaveBeenCalledOnce();
  });
});

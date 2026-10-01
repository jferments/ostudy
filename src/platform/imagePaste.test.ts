import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ImagePasteSession } from "./imagePaste";

const mocks = vi.hoisted(() => ({ read: vi.fn(), report: vi.fn() }));
vi.mock("../ipc/diagnostics", () => ({ reportFrontendFailure: mocks.report }));
vi.mock("./imageInput", async (original) => ({
  ...await original<typeof import("./imageInput")>(), readClipboardImageFiles: mocks.read,
}));

const disposers: Array<() => void> = [];
function image(name = "screenshot.png") { return new File(["pixels"], name, { type: "image/png" }); }
function shortcut(target: Window | HTMLElement = window, options: KeyboardEventInit = {}) {
  target.dispatchEvent(new KeyboardEvent("keydown", { key: "v", ctrlKey: true, bubbles: true, ...options }));
}
function paste(files: File[], target: Window | HTMLElement = window) {
  const event = new Event("paste", { cancelable: true, bubbles: true });
  Object.defineProperty(event, "clipboardData", { value: { files, items: [] } });
  target.dispatchEvent(event);
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
function bind(accept: (files: File[]) => void, session = new ImagePasteSession(),
  onError?: (message: string | null) => void, scope?: HTMLElement) {
  const dispose = session.bind(accept, window, onError, scope);
  disposers.push(dispose);
  return { session, dispose };
}

beforeEach(() => { vi.useFakeTimers(); mocks.read.mockReset().mockResolvedValue([]); mocks.report.mockReset(); });
afterEach(() => {
  for (const dispose of disposers.splice(0)) dispose();
  document.body.replaceChildren();
  vi.useRealTimers();
});

describe("image paste gesture and owner lifetime", () => {
  it("starts clipboard.read synchronously inside the shortcut and lets native paste claim it once", async () => {
    const accept = vi.fn();
    bind(accept);
    shortcut();
    // WebKit user activation must not be deferred to a timer or a promise callback.
    expect(mocks.read).toHaveBeenCalledOnce();
    expect(paste([image()]).defaultPrevented).toBe(true);
    await vi.runAllTimersAsync();
    expect(accept).toHaveBeenCalledOnce();
  });

  it("ignores a pending read after native paste claims the gesture", async () => {
    const read = deferred<File[]>();
    mocks.read.mockReturnValue(read.promise);
    const accept = vi.fn();
    bind(accept);
    shortcut();
    paste([image()]);
    read.resolve([image()]);
    await read.promise;
    expect(accept).toHaveBeenCalledOnce();
  });

  it("does not add a late native event again after the async read already completed", async () => {
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

  it("retains separate rapid screenshots in gesture order even when reads finish out of order", async () => {
    const first = deferred<File[]>();
    const second = deferred<File[]>();
    mocks.read.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    const accept = vi.fn();
    bind(accept);
    shortcut();
    shortcut();
    expect(mocks.read).toHaveBeenCalledTimes(2);
    const a = image("first.png");
    const b = image("second.png");
    second.resolve([b]);
    await second.promise;
    expect(accept).not.toHaveBeenCalled();
    first.resolve([a]);
    await first.promise;
    expect(accept.mock.calls).toEqual([[[a]], [[b]]]);
    paste([b]);
    expect(accept).toHaveBeenCalledTimes(2);
  });

  it("releases later screenshots when an earlier read has no supported image", async () => {
    const first = deferred<File[]>();
    mocks.read.mockReturnValueOnce(first.promise).mockResolvedValueOnce([image("second.png")]);
    const accept = vi.fn();
    bind(accept);
    shortcut();
    shortcut();
    await vi.runAllTimersAsync();
    expect(accept).not.toHaveBeenCalled();
    first.resolve([]);
    await first.promise;
    expect(accept.mock.calls[0]?.[0][0].name).toBe("second.png");
  });

  it("discards an unresolved read when its acquisition owner leaves", async () => {
    const read = deferred<File[]>();
    mocks.read.mockReturnValueOnce(read.promise);
    const accept = vi.fn();
    const { dispose } = bind(accept);
    shortcut();
    dispose();
    read.resolve([image()]);
    await read.promise;
    expect(accept).not.toHaveBeenCalled();
  });

  it("rejects a late read after source rebinding without cancelling accepted jobs", async () => {
    const read = deferred<File[]>();
    mocks.read.mockReturnValueOnce(read.promise);
    const oldSource = vi.fn();
    const { session, dispose } = bind(oldSource);
    shortcut();
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

  it("allows later context-menu and multi-image pastes and leaves text-only paste untouched", async () => {
    mocks.read.mockResolvedValue([image()]);
    const accept = vi.fn();
    bind(accept);
    shortcut();
    await vi.runAllTimersAsync();
    window.dispatchEvent(new Event("contextmenu"));
    const images = [image("a.png"), image("b.png")];
    paste(images);
    expect(accept).toHaveBeenLastCalledWith(images);
    expect(accept).toHaveBeenCalledTimes(2);
    expect(paste([]).defaultPrevented).toBe(false);
  });

  it("does not steal prompt text or paste gestures belonging to another overlay", () => {
    const root = document.createElement("div");
    const prompt = document.createElement("textarea");
    const destination = document.createElement("textarea");
    destination.setAttribute("data-image-paste-target", "");
    const outside = document.createElement("button");
    root.append(prompt, destination);
    document.body.append(root, outside);
    const accept = vi.fn();
    bind(accept, new ImagePasteSession(), undefined, root);
    shortcut(prompt);
    expect(paste([image()], prompt).defaultPrevented).toBe(false);
    shortcut(outside);
    expect(paste([image()], outside).defaultPrevented).toBe(false);
    expect(mocks.read).not.toHaveBeenCalled();
    shortcut(destination);
    expect(mocks.read).toHaveBeenCalledOnce();
    expect(paste([image()], destination).defaultPrevented).toBe(true);
    expect(accept).toHaveBeenCalledOnce();
  });

  it("supports Cmd+V and explicit Paste without accepting key repeats", async () => {
    mocks.read.mockResolvedValue([image()]);
    const accept = vi.fn();
    const { session } = bind(accept);
    shortcut(window, { repeat: true });
    expect(mocks.read).not.toHaveBeenCalled();
    shortcut(window, { ctrlKey: false, metaKey: true });
    expect(mocks.read).toHaveBeenCalledOnce();
    await vi.runAllTimersAsync();
    session.paste();
    expect(mocks.read).toHaveBeenCalledTimes(2);
    await vi.runAllTimersAsync();
    expect(accept).toHaveBeenCalledTimes(2);
  });

  it("drops unresolved reads on window blur and reports only safe clipboard errors", async () => {
    const read = deferred<File[]>();
    mocks.read.mockReturnValueOnce(read.promise);
    const accept = vi.fn();
    const error = vi.fn();
    const { session } = bind(accept, new ImagePasteSession(), error);
    shortcut();
    window.dispatchEvent(new Event("blur"));
    read.resolve([image()]);
    await read.promise;
    expect(accept).not.toHaveBeenCalled();
    const denied = new DOMException("private clipboard metadata", "NotAllowedError");
    mocks.read.mockRejectedValueOnce(denied);
    session.paste();
    await vi.runAllTimersAsync();
    expect(mocks.report).toHaveBeenCalledWith("clipboard_image_read_failed", denied);
    expect(error).toHaveBeenLastCalledWith(expect.stringContaining("Clipboard access was unavailable"));
    expect(JSON.stringify(error.mock.calls)).not.toContain("private clipboard metadata");
    paste([image()]);
    expect(accept).toHaveBeenCalledOnce();
    expect(error).toHaveBeenLastCalledWith(null);
  });

  it("explains an empty explicit Paste without preventing ordinary text paste", async () => {
    const error = vi.fn();
    const { session } = bind(vi.fn(), new ImagePasteSession(), error);
    session.paste();
    await vi.runAllTimersAsync();
    expect(error).toHaveBeenLastCalledWith(expect.stringContaining("No image was available"));
    expect(paste([]).defaultPrevented).toBe(false);
  });
});

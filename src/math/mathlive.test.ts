import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mountFormulaInput, type FormulaInput } from "./mathlive";

// A vendor stub isolates the lifecycle/clipboard contract; browser keyboard
// rendering and physical-device interaction remain separate acceptance work.
vi.mock("mathlive", () => {
  class TestField extends HTMLElement {
    static fontsDirectory: string | null = "https://invalid.example/fonts";
    static soundsDirectory: string | null = "https://invalid.example/sounds";
    static computeEngine: unknown = {};
    static openUrl = vi.fn();
    static restoreFocusWhenDocumentFocused = true;
    value = "";
    mathVirtualKeyboardPolicy = "auto";
    menuItems: unknown[] = [{}];
    insert(source: string) { this.value += source; }
  }
  customElements.define("test-formula-input", TestField);
  return { MathfieldElement: TestField };
});

class TestKeyboard extends EventTarget {
  container: HTMLElement = document.body;
  visible = false;
  boundingRect = new DOMRect(0, 0, 320, 180);
  show() { this.visible = true; this.dispatchEvent(new Event("geometrychange")); }
  hide() { this.visible = false; this.dispatchEvent(new Event("geometrychange")); }
}
let keyboard: TestKeyboard;
let host: HTMLDivElement;
let keyboardHost: HTMLDivElement;
const mounted: FormulaInput[] = [];
beforeEach(() => {
  keyboard = new TestKeyboard();
  Object.defineProperty(window, "mathVirtualKeyboard", { configurable: true, value: keyboard });
  host = document.createElement("div"); keyboardHost = document.createElement("div");
  document.body.append(host, keyboardHost);
});
afterEach(() => {
  mounted.splice(0).forEach((input) => input.dispose());
  host.remove(); keyboardHost.remove();
});
async function mount(source = "x") {
  const controller = new AbortController();
  const onChange = vi.fn(); const onError = vi.fn();
  const input = await mountFormulaInput({ host, keyboardHost, source, signal: controller.signal, onChange, onError });
  expect(input).not.toBeNull(); mounted.push(input!);
  return { input: input!, controller, onChange, onError };
}

describe("MathLive application port", () => {
  it("does not attach or claim the keyboard when loading finishes after dismissal", async () => {
    const controller = new AbortController();
    const pending = mountFormulaInput({ host, keyboardHost, source: "", signal: controller.signal, onChange: vi.fn(), onError: vi.fn() });
    controller.abort();
    expect(await pending).toBeNull();
    expect(host.childElementCount).toBe(0);
    expect(keyboard.container).toBe(document.body);
    await mount(); // The dismissed load did not retain the exclusive owner.
  });

  it("owns and releases the contained keyboard, listeners and field without normalizing the initial draft", async () => {
    const { input, controller, onChange } = await mount(" x ");
    const { MathfieldElement } = await import("mathlive");
    expect(MathfieldElement.fontsDirectory).toBeNull();
    expect(MathfieldElement.soundsDirectory).toBeNull();
    expect(MathfieldElement.computeEngine).toBeNull();
    expect(onChange).not.toHaveBeenCalled();
    expect(keyboard.container).toBe(keyboardHost);
    input.toggleKeyboard();
    expect(keyboardHost.style.height).toBe("180px");
    expect(input.hideKeyboard()).toBe(true);
    expect(input.hideKeyboard()).toBe(false);
    const field = host.firstElementChild!;
    controller.abort(); input.dispose();
    field.dispatchEvent(new Event("input"));
    expect(onChange).not.toHaveBeenCalled();
    expect(host.childElementCount).toBe(0);
    expect(keyboard.container).toBe(document.body);
    expect(keyboard.visible).toBe(false);
    await mount();
  });

  it("admits plain formula paste but blocks HTML-only, oversized and active source", async () => {
    const { onChange, onError } = await mount("x");
    const field = host.firstElementChild!;
    for (const source of ["", String.raw`\href{https://invalid.example}{x}`, "x".repeat(16385)]) {
      const event = new Event("paste", { bubbles: true, cancelable: true });
      Object.defineProperty(event, "clipboardData", { value: { getData: (format: string) => format === "text/plain" ? source : "<img src=x>" } });
      field.dispatchEvent(event);
      expect(event.defaultPrevented).toBe(true);
    }
    expect(onChange).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledTimes(3);
    const paste = new Event("paste", { bubbles: true, cancelable: true });
    Object.defineProperty(paste, "clipboardData", { value: { getData: () => "+1" } });
    field.dispatchEvent(paste);
    expect(onChange).toHaveBeenLastCalledWith("x+1");
  });

  it("rejects a second concurrent owner without disturbing the first dialog", async () => {
    const { input } = await mount();
    input.toggleKeyboard();
    await expect(mountFormulaInput({ host, keyboardHost, source: "y", signal: new AbortController().signal, onChange: vi.fn(), onError: vi.fn() }))
      .rejects.toThrow("already open");
    expect(keyboard.visible).toBe(true);
    expect(host.childElementCount).toBe(1);
  });
});

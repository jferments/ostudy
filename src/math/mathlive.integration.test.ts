import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mountFormulaInput, type FormulaInput } from "./mathlive";

// Node resolves MathLive's SSR-only export by default. Select the package's
// actual browser build for this jsdom integration, not a replacement class.
vi.mock("mathlive", async () => {
  const { createRequire } = await import("node:module");
  const { dirname, join } = await import("node:path");
  const { pathToFileURL } = await import("node:url");
  const directory = dirname(createRequire(import.meta.url).resolve("mathlive"));
  return import(/* @vite-ignore */ pathToFileURL(join(directory, "mathlive.min.mjs")).href);
});

// Exercise the actual pinned MathLive custom element. Only browser layout APIs
// absent from jsdom are shimmed: mounting/menu/value/insert are not vendor stubs.
let input: FormulaInput | null = null;
let host: HTMLDivElement;
let keyboardHost: HTMLDivElement;
let controller: AbortController;
const originalFonts = Object.getOwnPropertyDescriptor(document, "fonts");
const originalScrollIntoView = Object.getOwnPropertyDescriptor(
  HTMLElement.prototype,
  "scrollIntoView",
);
beforeEach(() => {
  controller = new AbortController();
  // jsdom has no FontFaceSet. MathLive listens for browser font readiness even
  // with its dynamic loader disabled; do not stub its field/model to bypass it.
  const fonts = Object.assign(new EventTarget(), { status: "loaded", check: () => true });
  Object.defineProperty(fonts, "ready", { value: Promise.resolve(fonts) });
  Object.defineProperty(document, "fonts", { configurable: true, value: fonts });
  // MathLive scrolls a changed selection into view. Browsers provide this API;
  // jsdom does not, so only this layout side effect is shimmed for integration.
  Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
    configurable: true,
    value: vi.fn(),
  });
  vi.stubGlobal("matchMedia", vi.fn((media: string) => ({ matches: false, media,
    addEventListener: vi.fn(), removeEventListener: vi.fn(), addListener: vi.fn(), removeListener: vi.fn() })));
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  host = document.createElement("div"); keyboardHost = document.createElement("div");
  document.body.append(host, keyboardHost);
});
afterEach(() => {
  controller.abort();
  input?.dispose(); input = null;
  host.remove(); keyboardHost.remove();
  if (originalFonts) Object.defineProperty(document, "fonts", originalFonts);
  else Reflect.deleteProperty(document, "fonts");
  if (originalScrollIntoView) {
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", originalScrollIntoView);
  } else {
    Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
  }
  vi.unstubAllGlobals();
});

it("mounts real MathLive, wraps a selection with a visual template, edits, and reopens", async () => {
  const onChange = vi.fn();
  input = await mountFormulaInput({ host, keyboardHost, source: "x^2+7x+3",
    signal: controller.signal, onChange, onError: vi.fn() });
  expect(input).not.toBeNull();
  const field = host.querySelector("math-field") as import("mathlive").MathfieldElement;
  expect(field).not.toBeNull();
  expect(field.menuItems).toEqual([]);
  expect(field.getValue()).toBe("x^2+7x+3");
  expect(onChange).not.toHaveBeenCalled();

  field.executeCommand("selectAll");
  expect(input!.applyTemplate(String.raw`\frac{#@}{17}`)).toBe(true);
  const fraction = field.getValue();
  expect(fraction).toContain(String.raw`\frac`);
  expect(fraction).toContain("x^2");
  expect(fraction).toContain("7x");
  expect(fraction).toContain("17");
  expect(fraction).not.toContain(String.raw`\left(`);
  expect(onChange).toHaveBeenCalled();

  field.insert("+1", { insertionMode: "replaceAll", format: "latex" });
  field.dispatchEvent(new Event("input"));
  expect(onChange).toHaveBeenLastCalledWith("+1");
  input!.dispose();
  expect(host.childElementCount).toBe(0);
  input = await mountFormulaInput({ host, keyboardHost, source: "x", signal: controller.signal, onChange, onError: vi.fn() });
  expect(input).not.toBeNull();
  expect(host.querySelector("math-field")).not.toBeNull();
});

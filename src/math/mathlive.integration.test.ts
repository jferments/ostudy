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
beforeEach(() => {
  vi.stubGlobal("matchMedia", vi.fn((media: string) => ({ matches: false, media,
    addEventListener: vi.fn(), removeEventListener: vi.fn(), addListener: vi.fn(), removeListener: vi.fn() })));
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  host = document.createElement("div"); keyboardHost = document.createElement("div");
  document.body.append(host, keyboardHost);
});
afterEach(() => {
  input?.dispose(); input = null;
  host.remove(); keyboardHost.remove();
  vi.unstubAllGlobals();
});

it("mounts the real mathfield before configuring its menu, edits a fraction, and reopens", async () => {
  const onChange = vi.fn();
  input = await mountFormulaInput({ host, keyboardHost, source: String.raw`\frac{a}{b}`,
    signal: new AbortController().signal, onChange, onError: vi.fn() });
  expect(input).not.toBeNull();
  const field = host.querySelector("math-field") as import("mathlive").MathfieldElement;
  expect(field).not.toBeNull();
  expect(field.menuItems).toEqual([]);
  expect(field.getValue()).toBe(String.raw`\frac{a}{b}`);
  expect(onChange).not.toHaveBeenCalled();
  field.insert("+1", { insertionMode: "replaceAll", format: "latex" });
  field.dispatchEvent(new Event("input"));
  expect(onChange).toHaveBeenLastCalledWith("+1");
  input!.dispose();
  expect(host.childElementCount).toBe(0);
  input = await mountFormulaInput({ host, keyboardHost, source: "x", signal: new AbortController().signal, onChange, onError: vi.fn() });
  expect(input).not.toBeNull();
  expect(host.querySelector("math-field")).not.toBeNull();
});

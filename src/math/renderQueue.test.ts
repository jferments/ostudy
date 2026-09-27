import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { scheduleFormula } from "./renderQueue";
import { renderFormula } from "./render";

import { requiredTestValue as required } from "../test/factories";
vi.mock("./render", () => ({ renderFormula: vi.fn() }));
let frames: FrameRequestCallback[] = [];
let disposers: (() => void)[] = [];
let visible: IntersectionObserverCallback | null = null;
const observe = vi.fn();
const unobserve = vi.fn();
const disconnect = vi.fn();
function host(source = "x") {
  const element = document.createElement("span");
  document.body.append(element);
  disposers.push(scheduleFormula(element, source, false));
  return element;
}
function tick() {
  const pending = frames;
  frames = [];
  for (const callback of pending) callback(0);
}
beforeEach(() => {
  frames = []; disposers = []; visible = null;
  vi.clearAllMocks();
  vi.stubGlobal("requestAnimationFrame", vi.fn((callback: FrameRequestCallback) => { frames.push(callback); return frames.length; }));
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
  vi.stubGlobal("IntersectionObserver", undefined);
});
afterEach(() => {
  for (const dispose of disposers) dispose();
  document.body.replaceChildren();
  vi.unstubAllGlobals();
});

describe("bounded mathematical presentation queue", () => {
  it("spreads dense answers across frames instead of making their Markdown literal", () => {
    for (let i = 0; i < 65; i += 1) host(`x_${i}`);
    expect(renderFormula).not.toHaveBeenCalled();
    tick();
    expect(renderFormula).toHaveBeenCalledTimes(4);
    for (let i = 0; i < 20; i += 1) tick();
    expect(renderFormula).toHaveBeenCalledTimes(65);
    expect(document.querySelector("[aria-busy]")).toBeNull();
  });

  it("does no typesetting for an offscreen equation until it approaches the viewport", () => {
    let instance: IntersectionObserver | null = null;
    vi.stubGlobal("IntersectionObserver", class implements IntersectionObserver {
      readonly root = null;
      readonly rootMargin = "300px";
      readonly thresholds = [0];
      takeRecords = () => [];
      constructor(callback: IntersectionObserverCallback) { visible = callback; instance = this; }
      observe = observe; unobserve = unobserve; disconnect = disconnect;
    });
    const element = host();
    tick();
    expect(renderFormula).not.toHaveBeenCalled();
    expect(element.textContent).toBe(String.raw`\(x\)`);
    const bounds = element.getBoundingClientRect();
    const entry: IntersectionObserverEntry = {
      target: element, isIntersecting: true, intersectionRatio: 1, time: 0,
      boundingClientRect: bounds, intersectionRect: bounds, rootBounds: null,
    };
    visible!([entry], instance!);
    tick();
    expect(renderFormula).toHaveBeenCalledExactlyOnceWith(element, "x", false);
    expect(disconnect).toHaveBeenCalled();
  });

  it("cancels obsolete queued formulas without painting stale replies", () => {
    const element = host("old");
    required(disposers[0], "first render disposer")();
    disposers.push(scheduleFormula(element, "new", true));
    tick();
    expect(renderFormula).toHaveBeenCalledExactlyOnceWith(element, "new", true);
  });

  it("bounds source bytes per frame as well as equation count", () => {
    host("x".repeat(9_000)); host("y".repeat(9_000));
    tick(); expect(renderFormula).toHaveBeenCalledTimes(1);
    tick(); expect(renderFormula).toHaveBeenCalledTimes(2);
  });
});

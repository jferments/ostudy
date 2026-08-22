import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

if (typeof window.requestAnimationFrame !== "function") {
  window.requestAnimationFrame = (callback: FrameRequestCallback): number =>
    window.setTimeout(() => callback(performance.now()), 0);
}
if (typeof window.cancelAnimationFrame !== "function") {
  window.cancelAnimationFrame = (handle: number): void => window.clearTimeout(handle);
}

function assertBrowserStorage(storage: Storage, name: string): void {
  const key = `__ostudy_${name}_probe__`;
  storage.setItem(key, "ready");
  if (storage.getItem(key) !== "ready") {
    throw new Error(`${name} is not a functional browser Storage implementation.`);
  }
  storage.removeItem(key);
  storage.clear();
}

assertBrowserStorage(window.localStorage, "localStorage");
assertBrowserStorage(window.sessionStorage, "sessionStorage");

afterEach(() => {
  cleanup();
});

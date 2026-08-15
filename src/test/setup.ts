import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

if (typeof window.requestAnimationFrame !== "function") {
  window.requestAnimationFrame = (callback: FrameRequestCallback): number =>
    window.setTimeout(() => callback(performance.now()), 0);
}
if (typeof window.cancelAnimationFrame !== "function") {
  window.cancelAnimationFrame = (handle: number): void => window.clearTimeout(handle);
}

afterEach(() => {
  cleanup();
});

import { afterEach, describe, expect, it, vi } from "vitest";

import {
  dispatchPlatformBack,
  platformBackHandlerCountForTests,
  registerPlatformBackHandler,
} from "./backNavigation";

const cleanups: Array<() => void> = [];

afterEach(() => {
  while (cleanups.length > 0) cleanups.pop()?.();
  expect(platformBackHandlerCountForTests()).toBe(0);
});

describe("platform Back dispatcher", () => {
  it("offers Back to the newest registered layer first", () => {
    const older = vi.fn(() => true);
    const newer = vi.fn(() => true);
    cleanups.push(registerPlatformBackHandler(older));
    cleanups.push(registerPlatformBackHandler(newer));

    expect(dispatchPlatformBack()).toBe(true);
    expect(newer).toHaveBeenCalledOnce();
    expect(older).not.toHaveBeenCalled();
  });

  it("falls through when a handler explicitly declines the action", () => {
    const older = vi.fn(() => true);
    const newer = vi.fn(() => false);
    cleanups.push(registerPlatformBackHandler(older));
    cleanups.push(registerPlatformBackHandler(newer));

    expect(dispatchPlatformBack()).toBe(true);
    expect(newer).toHaveBeenCalledOnce();
    expect(older).toHaveBeenCalledOnce();
  });

  it("reports an unconsumed Back action when the stack is empty", () => {
    expect(dispatchPlatformBack()).toBe(false);
  });
});

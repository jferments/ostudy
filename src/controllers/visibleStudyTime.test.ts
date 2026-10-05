import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as platformIpc from "../ipc/platform";
import type { LifecyclePhase, LifecycleSnapshot } from "../ipc/platform";
import {
  activeTimeForReview,
  MAX_ACTIVE_TIME_MS,
  useVisibleStudyTime,
  VisibleStudyTime,
} from "./visibleStudyTime";

vi.mock("../ipc/platform", () => ({ listenToLifecycle: vi.fn() }));

let visibility: DocumentVisibilityState = "visible";

function setVisibility(state: DocumentVisibilityState): void {
  visibility = state;
  act(() => {
    document.dispatchEvent(new Event("visibilitychange"));
  });
}

function lifecycle(phase: LifecyclePhase): LifecycleSnapshot {
  return { schemaVersion: 1, phase, observedAtUtcMillis: 0 };
}

beforeEach(() => {
  vi.useFakeTimers();
  visibility = "visible";
  vi.spyOn(document, "visibilityState", "get").mockImplementation(() => visibility);
  // Without a native bridge, document visibility is the only signal.
  vi.mocked(platformIpc.listenToLifecycle).mockRejectedValue(new Error("No native bridge"));
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useVisibleStudyTime", () => {
  it("pauses while the document is hidden", async () => {
    const { result } = renderHook(() => useVisibleStudyTime());
    await act(async () => undefined);
    result.current.restart();

    vi.advanceTimersByTime(3_000);
    setVisibility("hidden");
    vi.advanceTimersByTime(60_000);
    expect(result.current.elapsedMs()).toBe(3_000);
    setVisibility("visible");
    vi.advanceTimersByTime(2_000);

    expect(result.current.elapsedMs()).toBe(5_000);
  });

  it("starts paused when mounted hidden and a restart waits for the card to be visible", async () => {
    visibility = "hidden";
    const { result } = renderHook(() => useVisibleStudyTime());
    await act(async () => undefined);
    vi.advanceTimersByTime(10_000);
    result.current.restart();
    vi.advanceTimersByTime(10_000);
    expect(result.current.elapsedMs()).toBe(0);

    setVisibility("visible");
    vi.advanceTimersByTime(1_250);
    expect(result.current.elapsedMs()).toBe(1_250);
  });

  it("pauses between platform background and foreground events and stops listening on unmount", async () => {
    let emit: (snapshot: LifecycleSnapshot) => void = () => undefined;
    const unlisten = vi.fn();
    vi.mocked(platformIpc.listenToLifecycle).mockImplementation(async (listener) => {
      emit = listener;
      return unlisten;
    });
    const { result, unmount } = renderHook(() => useVisibleStudyTime());
    await act(async () => undefined);
    result.current.restart();

    vi.advanceTimersByTime(1_000);
    emit(lifecycle("background"));
    vi.advanceTimersByTime(30_000);
    emit(lifecycle("foreground"));
    vi.advanceTimersByTime(500);
    emit(lifecycle("termination_requested"));
    vi.advanceTimersByTime(8_000);
    emit(lifecycle("foreground"));
    vi.advanceTimersByTime(250);
    expect(result.current.elapsedMs()).toBe(1_750);

    unmount();
    expect(unlisten).toHaveBeenCalledOnce();
  });
});

describe("VisibleStudyTime", () => {
  it("keeps the clock stopped until every pause reason clears and restarts from zero", () => {
    let now = 0;
    const timer = new VisibleStudyTime(() => now);
    now = 2_000;
    timer.pause("editor");
    now = 5_000;
    timer.pause("hidden");
    timer.resume("editor");
    now = 9_000;
    expect(timer.elapsedMs()).toBe(2_000);
    timer.resume("hidden");
    timer.resume("hidden");
    now = 9_250;
    expect(timer.elapsedMs()).toBe(2_250);

    timer.restart();
    now = 9_300;
    expect(timer.elapsedMs()).toBe(50);
  });
});

describe("activeTimeForReview", () => {
  it("records whole visible milliseconds and reports unusable measurements as unknown", () => {
    expect(activeTimeForReview(0)).toBe(0);
    expect(activeTimeForReview(1_234.4)).toBe(1_234);
    expect(activeTimeForReview(MAX_ACTIVE_TIME_MS)).toBe(MAX_ACTIVE_TIME_MS);
    expect(activeTimeForReview(MAX_ACTIVE_TIME_MS + 1)).toBeNull();
    expect(activeTimeForReview(-1)).toBeNull();
    expect(activeTimeForReview(Number.NaN)).toBeNull();
    expect(activeTimeForReview(Number.POSITIVE_INFINITY)).toBeNull();
  });
});

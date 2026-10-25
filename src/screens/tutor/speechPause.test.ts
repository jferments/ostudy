import { describe, expect, it } from "vitest";
import { createPauseDetector } from "./speechPause";

/** Feeds one level every 100 ms from `start` and returns when the pause was reported. */
function feed(levels: number[], pauseMs = 2000): number | null {
  const pause = createPauseDetector(pauseMs);
  for (const [index, level] of levels.entries()) {
    if (pause(level, index * 100)) return index * 100;
  }
  return null;
}

const quiet = (ms: number) => Array<number>(ms / 100).fill(0.01);
const talk = (ms: number) => Array<number>(ms / 100).fill(0.4);

describe("speech pause", () => {
  it("ends after speech followed by the chosen silence", () => {
    expect(feed([...quiet(500), ...talk(1500), ...quiet(3000)])).toBe(3900);
  });

  it("waits through silence before the learner starts speaking", () => {
    expect(feed(quiet(10_000))).toBeNull();
  });

  it("does not take a short click for speech", () => {
    expect(feed([...quiet(500), 0.5, ...quiet(5000)])).toBeNull();
  });

  it("keeps listening through pauses shorter than the chosen one", () => {
    expect(feed([...talk(1000), ...quiet(1500), ...talk(1000), ...quiet(1000)])).toBeNull();
  });

  it("measures speech against the room's own noise", () => {
    // A steady hum at 0.1 is not speech; talking well above it is.
    const hum = (ms: number) => Array<number>(ms / 100).fill(0.1);
    expect(feed(hum(5000))).toBeNull();
    expect(feed([...hum(500), ...Array<number>(10).fill(0.5), ...hum(2500)])).toBe(3400);
  });
});

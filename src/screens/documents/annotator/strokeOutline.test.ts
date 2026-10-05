import { describe, expect, it } from "vitest";

import type { InkShape } from "./annotationModel";
import { highlightPath, inkPath } from "./strokeOutline";

function ink(pressure: number, simulatePressure = false): InkShape {
  const points = Array.from({ length: 11 }, (_, index) => [index * 10, 50, pressure]).flat();
  return { kind: "ink", points, color: "#1f2328", strokeWidth: 4, simulatePressure };
}

/** How far the outline reaches above and below a horizontal stroke. */
function thickness(path: string): number {
  const ys = [...path.matchAll(/(-?\d+\.\d+),(-?\d+\.\d+)/g)].map((match) => Number(match[2]));
  return Math.max(...ys) - Math.min(...ys);
}

describe("stroke outlines", () => {
  it("closes a pen stroke's outline and widens it with pressure", () => {
    const light = inkPath(ink(0.2));
    const firm = inkPath(ink(1));
    expect(light).toMatch(/^M.* Q.*Z$/);
    expect(thickness(firm)).toBeGreaterThan(thickness(light));
    expect(thickness(inkPath(ink(0.5, true)))).toBeGreaterThan(0);
  });

  it("draws a dot for a single tap of the pen", () => {
    expect(inkPath({ ...ink(0.5), points: [10, 10, 0.5] })).not.toBe("");
  });

  it("draws highlights as an even path through their points", () => {
    expect(highlightPath({ kind: "highlight", points: [0, 0, 10, 0, 20, 5], color: "#fde047", strokeWidth: 16 }))
      .toBe("M0.00,0.00 L10.00,0.00 L20.00,5.00");
    expect(highlightPath({ kind: "highlight", points: [10, 20], color: "#fde047", strokeWidth: 16 }))
      .toBe("M10.00,20.00 L10.00,20.00");
    expect(highlightPath({ kind: "highlight", points: [], color: "#fde047", strokeWidth: 16 })).toBe("");
  });
});

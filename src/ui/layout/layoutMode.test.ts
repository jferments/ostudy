import { describe, expect, it } from "vitest";

import {
  createLayoutDiagnostics,
  resolveLayoutMode,
} from "./layoutMode";

describe("responsive layout modes", () => {
  it.each([
    [360, "phone"],
    [600, "phone"],
    [760, "phone"],
    [761, "tablet"],
    [800, "tablet"],
    [824, "tablet"],
    [1024, "tablet"],
    [1179, "tablet"],
    [1180, "wide"],
    [1440, "wide"],
  ] as const)("maps %ipx to %s", (width, mode) => {
    expect(resolveLayoutMode(width)).toBe(mode);
  });

  it("reports orientation and scale pressure without card or document content", () => {
    expect(
      createLayoutDiagnostics({
        width: 824,
        height: 1200,
        devicePixelRatio: 2,
        uiScalePercent: 140,
        input: "coarse",
      }),
    ).toEqual({
      width: 824,
      height: 1200,
      devicePixelRatio: 2,
      uiScalePercent: 140,
      input: "coarse",
      mode: "tablet",
      orientation: "portrait",
      effectiveWidthAtScale: 589,
    });
  });
});

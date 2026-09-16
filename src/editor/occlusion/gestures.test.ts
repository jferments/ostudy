import { describe, expect, it } from "vitest";

import { FIT_VIEWPORT, imageTransform, toScreenPoint } from "./geometry";
import { OcclusionGestures, type GestureContext, type PointerSample } from "./gestures";

const mask = { id: "a", rect: { x: 0.2, y: 0.2, width: 0.2, height: 0.2 } };
const context: GestureContext = {
  image: { width: 1000, height: 1000 }, frame: { width: 500, height: 500 },
  viewport: FIT_VIEWPORT, masks: [mask], tool: "rectangle",
};
function point(x: number, y: number, id = 1): PointerSample {
  return { ...toScreenPoint({ x, y }, imageTransform(context.viewport, context.image, context.frame)), id };
}

describe("occlusion gesture transactions", () => {
  it("commits one rectangle at pointer-up, never at a preview move", () => {
    const gestures = new OcclusionGestures();
    expect(gestures.down(point(0.1, 0.1), null, context).mutation).toBeUndefined();
    for (const position of [0.2, 0.3, 0.4]) {
      const preview = gestures.move(point(position, position));
      expect(preview.mutation).toBeUndefined();
      expect(preview.preview).not.toBeNull();
    }
    const result = gestures.up(point(0.4, 0.4));
    expect(result.mutation?.kind).toBe("create");
    expect(result.preview).toBeNull();
    expect(gestures.up(point(0.4, 0.4)).mutation).toBeUndefined();
  });

  it("cancels an unfinished region without changing authored content", () => {
    const gestures = new OcclusionGestures();
    gestures.down(point(0.2, 0.2), null, context);
    gestures.move(point(0.7, 0.7));
    expect(gestures.cancel().preview).toBeNull();
    expect(gestures.up(point(0.7, 0.7)).mutation).toBeUndefined();
    expect(context.masks).toEqual([mask]);
  });

  it("turns a second finger into viewport navigation rather than an extra region", () => {
    const gestures = new OcclusionGestures();
    gestures.down(point(0.2, 0.2), null, context);
    gestures.move(point(0.4, 0.4));
    expect(gestures.down(point(0.7, 0.7, 2), null, context).preview).toBeNull();
    expect(gestures.move(point(0.9, 0.9, 2)).viewport?.zoom).toBeGreaterThan(1);
    expect(gestures.up(point(0.9, 0.9, 2)).mutation).toBeUndefined();
    expect(gestures.move(point(0.8, 0.8)).preview).toBeNull();
    expect(gestures.up(point(0.8, 0.8)).mutation).toBeUndefined();
    expect(gestures.isActive()).toBe(false);
  });

  it("supports two separate taps as a rectangle alternative", () => {
    const gestures = new OcclusionGestures();
    const corners = { ...context, tool: "corners" as const };
    gestures.down(point(0.2, 0.3), null, corners);
    expect(gestures.up(point(0.2, 0.3)).firstCorner).not.toBeNull();
    gestures.down(point(0.6, 0.7), null, corners);
    const result = gestures.up(point(0.6, 0.7));
    expect(result.firstCorner).toBeNull();
    expect(result.mutation?.kind).toBe("create");
    expect(result.mutation && "rectangle" in result.mutation && result.mutation.rectangle.width).toBeCloseTo(0.4, 10);
  });

  it("moving a region emits a compare-and-set edit with the starting geometry", () => {
    const gestures = new OcclusionGestures();
    const selected = { ...context, tool: "select" as const };
    expect(gestures.down(point(0.3, 0.3), { maskId: mask.id }, selected).select).toBe(mask.id);
    const result = gestures.up(point(0.5, 0.5));
    expect(result.mutation).toMatchObject({ kind: "change", maskId: mask.id, before: mask.rect });
    expect(result.mutation && "after" in result.mutation && result.mutation.after.x).toBeCloseTo(0.4, 10);
    expect(mask.rect.x).toBe(0.2);
  });

  it("resizing uses the selected handle and cancellation emits no geometry", () => {
    const gestures = new OcclusionGestures();
    gestures.down(point(0.4, 0.4), { maskId: mask.id, handle: "se" }, { ...context, tool: "select" });
    const resized = gestures.move(point(0.6, 0.6));
    expect(resized.preview?.rectangle.width).toBeCloseTo(0.4, 10);
    expect(resized.preview?.rectangle.x).toBe(0.2);
    gestures.cancel();
    expect(gestures.up(point(0.6, 0.6)).mutation).toBeUndefined();
  });

  it("ignores stray pointers, taps and clicks in the letterbox", () => {
    const gestures = new OcclusionGestures();
    expect(gestures.move(point(0.5, 0.5, 99)).mutation).toBeUndefined();
    gestures.down(point(0.2, 0.2), null, context);
    expect(gestures.up(point(0.2, 0.2)).mutation).toBeUndefined();
    gestures.down({ id: 2, x: 0, y: 0 }, null, context);
    expect(gestures.up(point(0.8, 0.8, 2)).mutation).toBeUndefined();
    expect(gestures.isActive()).toBe(false);
  });

  it("a third pointer cancels the active transform until all pointers lift", () => {
    const gestures = new OcclusionGestures();
    gestures.down(point(0.1, 0.1), null, context);
    gestures.down(point(0.5, 0.5, 2), null, context);
    gestures.down(point(0.7, 0.7, 3), null, context);
    expect(gestures.move(point(0.9, 0.9)).viewport).toBeUndefined();
    for (const id of [1, 2, 3]) expect(gestures.up(point(0.9, 0.9, id)).mutation).toBeUndefined();
    expect(gestures.isActive()).toBe(false);
  });
});

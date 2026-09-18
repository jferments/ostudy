import type { ImageMask, NormalizedRect } from "../../ipc/questions";
import {
  imageTransform, insideImage, panViewport, rectangleFromCorners, moveRectangle,
  resizeRectangle, sameRectangle, toImagePoint, zoomViewport,
  type ImageViewport, type Point, type ResizeHandle, type Size,
} from "./geometry";

export type OcclusionTool = "select" | "rectangle" | "corners" | "pan";
export interface PointerSample extends Point { id: number; }
export interface RegionHit { maskId: string; handle?: ResizeHandle; }
export interface GestureContext { image: Size; frame: Size; viewport: ImageViewport; masks: readonly ImageMask[]; tool: OcclusionTool; }
export type RegionMutation =
  | { kind: "create"; rectangle: NormalizedRect }
  | { kind: "change"; maskId: string; before: NormalizedRect; after: NormalizedRect };
export interface GestureResult {
  preview: { maskId: string | null; rectangle: NormalizedRect } | null;
  firstCorner: Point | null;
  viewport?: ImageViewport;
  select?: string | null;
  mutation?: RegionMutation;
}

type SingleGesture = {
  pointerId: number;
  start: Point;
  context: GestureContext;
} & (
  | { kind: "draw" | "corners" | "pan" }
  | { kind: "move"; mask: ImageMask }
  | { kind: "resize"; mask: ImageMask; handle: ResizeHandle }
);
type ActiveGesture = SingleGesture
  | { kind: "pinch"; ids: [number, number]; midpoint: Point; distance: number; context: GestureContext }
  | { kind: "wait"; untilClear?: boolean };

const THRESHOLD = 4;
const distance = (a: Point, b: Point) => Math.hypot(a.x - b.x, a.y - b.y);
const midpoint = (a: Point, b: Point): Point => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });

/** Pure gesture state machine. Only pointer-up emits geometry mutations.
 * A second pointer cancels an unfinished edit; it cannot turn into a rectangle.
 * The DOM adapter owns capture and cancellation, not the authored geometry.
 */
export class OcclusionGestures {
  private pointers = new Map<number, Point>();
  private active: ActiveGesture | null = null;
  private preview: GestureResult["preview"] = null;
  private corner: Point | null = null;

  snapshot(): GestureResult {
    return { preview: this.preview, firstCorner: this.corner };
  }

  down(pointer: PointerSample, hit: RegionHit | null, context: GestureContext): GestureResult {
    if (this.pointers.has(pointer.id)) return this.snapshot();
    this.pointers.set(pointer.id, { x: pointer.x, y: pointer.y });
    if (this.active?.kind === "wait" && this.active.untilClear) return this.snapshot();
    if (this.pointers.size > 2) {
      this.active = { kind: "wait", untilClear: true };
      this.preview = null;
      this.corner = null;
      return this.snapshot();
    }
    if (this.pointers.size === 2) {
      this.preview = null;
      this.corner = null;
      const entries = [...this.pointers.entries()];
      const [a, b] = entries;
      if (!a || !b) return this.snapshot();
      this.active = {
        kind: "pinch", ids: [a[0], b[0]], midpoint: midpoint(a[1], b[1]),
        distance: Math.max(1, distance(a[1], b[1])), context,
      };
      return this.snapshot();
    }
    const start = { x: pointer.x, y: pointer.y };
    const base = { pointerId: pointer.id, start, context };
    if (context.tool === "pan") {
      this.active = { ...base, kind: "pan" };
      return this.snapshot();
    }
    if (context.tool === "rectangle" || context.tool === "corners") {
      if (!insideImage(toImagePoint(start, imageTransform(context.viewport, context.image, context.frame)))) {
        this.active = { kind: "wait" };
        return this.snapshot();
      }
      this.active = { ...base, kind: context.tool === "rectangle" ? "draw" : "corners" };
      return this.snapshot();
    }
    const mask = context.masks.find((candidate) => candidate.id === hit?.maskId);
    if (mask) {
      const frozen = { ...mask, rect: { ...mask.rect } };
      this.active = hit?.handle
        ? { ...base, kind: "resize", mask: frozen, handle: hit.handle }
        : { ...base, kind: "move", mask: frozen };
      return { ...this.snapshot(), select: mask.id };
    }
    this.active = { ...base, kind: "pan" };
    return { ...this.snapshot(), select: null };
  }

  move(pointer: PointerSample): GestureResult {
    if (!this.pointers.has(pointer.id)) return this.snapshot();
    this.pointers.set(pointer.id, { x: pointer.x, y: pointer.y });
    const active = this.active;
    if (!active || active.kind === "wait") return this.snapshot();
    if (active.kind === "pinch") {
      const a = this.pointers.get(active.ids[0]);
      const b = this.pointers.get(active.ids[1]);
      if (!a || !b) return this.snapshot();
      const context = active.context;
      const center = midpoint(a, b);
      const zoom = context.viewport.zoom * distance(a, b) / active.distance;
      const zoomed = zoomViewport(context.viewport, context.image, context.frame, zoom, active.midpoint);
      return { ...this.snapshot(), viewport: panViewport(zoomed, context.image, context.frame, {
        x: center.x - active.midpoint.x, y: center.y - active.midpoint.y,
      }) };
    }
    if (active.pointerId !== pointer.id) return this.snapshot();
    const context = active.context;
    const delta = { x: pointer.x - active.start.x, y: pointer.y - active.start.y };
    if (distance(pointer, active.start) < THRESHOLD) {
      this.preview = null;
      return this.snapshot();
    }
    if (active.kind === "pan") {
      return { ...this.snapshot(), viewport: panViewport(context.viewport, context.image, context.frame, delta) };
    }
    const transform = imageTransform(context.viewport, context.image, context.frame);
    if (active.kind === "draw") {
      const rectangle = rectangleFromCorners(toImagePoint(active.start, transform), toImagePoint(pointer, transform), context.image);
      this.preview = rectangle ? { maskId: null, rectangle } : null;
    } else if (active.kind === "move" || active.kind === "resize") {
      const normalized = { x: delta.x / transform.width, y: delta.y / transform.height };
      const rectangle = active.kind === "move"
        ? moveRectangle(active.mask.rect, normalized)
        : resizeRectangle(active.mask.rect, active.handle, normalized, context.image);
      this.preview = { maskId: active.mask.id, rectangle };
    }
    return this.snapshot();
  }

  up(pointer: PointerSample): GestureResult {
    if (!this.pointers.has(pointer.id)) return this.snapshot();
    const result = this.move(pointer);
    const active = this.active;
    this.pointers.delete(pointer.id);
    let mutation: RegionMutation | undefined;
    if (active && active.kind !== "wait" && active.kind !== "pinch" && active.pointerId === pointer.id) {
      if (active.kind === "draw" && this.preview) {
        mutation = { kind: "create", rectangle: this.preview.rectangle };
      } else if ((active.kind === "move" || active.kind === "resize") && this.preview
        && !sameRectangle(active.mask.rect, this.preview.rectangle)) {
        mutation = { kind: "change", maskId: active.mask.id, before: active.mask.rect, after: this.preview.rectangle };
      } else if (active.kind === "corners" && distance(pointer, active.start) < THRESHOLD) {
        const context = active.context;
        const point = toImagePoint(pointer, imageTransform(context.viewport, context.image, context.frame));
        if (insideImage(point)) {
          if (this.corner) {
            const rectangle = rectangleFromCorners(this.corner, point, context.image);
            if (rectangle) mutation = { kind: "create", rectangle };
            this.corner = null;
          } else this.corner = point;
        }
      }
    }
    // Ending a pinch never resumes a one-finger geometry action.
    this.active = this.pointers.size > 0 ? { kind: "wait", untilClear: true } : null;
    this.preview = null;
    return { ...this.snapshot(), viewport: result.viewport, mutation };
  }

  cancel(): GestureResult {
    this.pointers.clear();
    this.active = null;
    this.preview = null;
    this.corner = null;
    return this.snapshot();
  }

  owns(pointerId: number): boolean { return this.pointers.has(pointerId); }
  isActive(): boolean { return this.active !== null; }
}

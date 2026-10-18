import { getStroke } from "perfect-freehand";

import type { HighlightShape, InkShape } from "../../../ipc/documentAnnotations";

// The one owner of perfect-freehand (see scripts/ui/import-policy.mjs). Strokes
// are stored as their input points, never as outlines, so a later renderer can
// redraw them without a migration.

function fixed(value: number): string {
  return value.toFixed(2);
}

function midpoint(a: ArrayLike<number>, b: ArrayLike<number>): string {
  return `${fixed(((a[0] ?? 0) + (b[0] ?? 0)) / 2)},${fixed(((a[1] ?? 0) + (b[1] ?? 0)) / 2)}`;
}

/** A closed SVG path around an outline: quadratic curves through the midpoints of its sides. */
function outlinePath(outline: ReadonlyArray<ArrayLike<number>>): string {
  const count = outline.length;
  const first = outline[0];
  const second = outline[1];
  if (!first || !second) return "";
  if (count < 4) {
    return `M${outline.map((point) => `${fixed(point[0] ?? 0)},${fixed(point[1] ?? 0)}`).join(" L")} Z`;
  }
  let path = `M${midpoint(first, second)} Q`;
  for (let index = 1; index <= count; index += 1) {
    const corner = outline[index % count] ?? first;
    const next = outline[(index + 1) % count] ?? first;
    path += `${fixed(corner[0] ?? 0)},${fixed(corner[1] ?? 0)} ${midpoint(corner, next)} `;
  }
  return `${path}Z`;
}

/**
 * The filled outline of a pen stroke. Recorded pressure sets its width; a
 * stroke made without pressure is shaped from its speed instead. `complete`
 * is false while the stroke is still being drawn.
 */
export function inkPath(shape: InkShape, complete = true): string {
  const points: number[][] = [];
  for (let index = 0; index + 2 < shape.points.length; index += 3) {
    points.push([shape.points[index] ?? 0, shape.points[index + 1] ?? 0, shape.points[index + 2] ?? 0.5]);
  }
  return outlinePath(getStroke(points, {
    size: shape.strokeWidth,
    thinning: shape.simulatePressure ? 0.35 : 0.6,
    smoothing: 0.5,
    streamline: 0.4,
    simulatePressure: shape.simulatePressure,
    last: complete,
  }));
}

/** A highlighter stroke is an even-width path; its look comes from the stroke style. */
export function highlightPath(shape: HighlightShape): string {
  const segments: string[] = [];
  for (let index = 0; index + 1 < shape.points.length; index += 2) {
    segments.push(`${fixed(shape.points[index] ?? 0)},${fixed(shape.points[index + 1] ?? 0)}`);
  }
  if (segments.length === 1) segments.push(segments[0] ?? "0,0");
  return segments.length === 0 ? "" : `M${segments.join(" L")}`;
}

/** A text highlight's boxes, one per line, as a single filled path. */
export function textHighlightPath(rects: readonly number[]): string {
  const boxes: string[] = [];
  for (let index = 0; index + 3 < rects.length; index += 4) {
    const x = rects[index] ?? 0;
    const y = rects[index + 1] ?? 0;
    boxes.push(`M${fixed(x)},${fixed(y)} h${fixed(rects[index + 2] ?? 0)} v${fixed(rects[index + 3] ?? 0)} h${fixed(-(rects[index + 2] ?? 0))} Z`);
  }
  return boxes.join(" ");
}

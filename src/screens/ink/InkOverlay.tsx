import type { InkMark } from "../../ipc/materialInk";
import { cn } from "../../ui/internal/cn";
import { ShapeGraphic } from "../documents/annotator/AnnotatorPage";

/**
 * The learner's marks on one designed page, drawn over its picture at the
 * page's size in points. It is a picture too: it takes no input.
 */
export function InkOverlay({ marks, width, height, className }: {
  marks: readonly InkMark[];
  width: number;
  height: number;
  className?: string;
}) {
  if (marks.length === 0) return null;
  return (
    <svg viewBox={`0 0 ${width} ${height}`} aria-hidden="true" data-ink-overlay=""
      className={cn("tw:pointer-events-none tw:absolute tw:inset-0 tw:size-full", className)}>
      {marks.map((mark) => mark.shape.kind === "text" && mark.shape.collapsed ? (
        <rect key={mark.id} x={mark.shape.x} y={mark.shape.y} width={18} height={18} rx={3} fill={mark.shape.color} />
      ) : mark.shape.kind === "text" ? (
        // Typed notes wrap at their width, as in the editor.
        <foreignObject key={mark.id} x={mark.shape.x} y={mark.shape.y} width={mark.shape.width}
          height={Math.max(1, height - mark.shape.y)}>
          <div style={{
            fontSize: mark.shape.fontSize, lineHeight: 1.25, padding: 4, whiteSpace: "pre-wrap", overflowWrap: "anywhere",
            color: mark.shape.sticky ? "#1f2328" : mark.shape.color,
            background: mark.shape.sticky ? mark.shape.color : undefined,
          }}>
            {mark.shape.text}
          </div>
        </foreignObject>
      ) : (
        <g key={mark.id} opacity={mark.shape.kind === "highlight" ? 0.75 : undefined}>
          <ShapeGraphic shape={mark.shape} />
        </g>
      ))}
    </svg>
  );
}

/** Marks by page key, earlier marks first. */
export function marksByPage(marks: readonly InkMark[]): ReadonlyMap<string, readonly InkMark[]> {
  const pages = new Map<string, InkMark[]>();
  for (const mark of marks) {
    const page = pages.get(mark.pageKey);
    if (page) page.push(mark);
    else pages.set(mark.pageKey, [mark]);
  }
  return pages;
}

import { useId } from "react";

import type { NotebookItem, NotebookPage, Paper } from "../../ipc/notebooks";
import { shapeBounds } from "../documents/annotator/annotationModel";
import { ShapeGraphic } from "../documents/annotator/AnnotatorPage";

export const PAPER_LABELS: Readonly<Record<Paper, string>> = {
  blank: "Blank",
  lined: "Lined",
  grid: "Grid",
  dots: "Dots",
};

/** Spacing of ruled lines, grid squares and dots, in points. */
const LINE_SPACING = 24;
const GRID_SPACING = 18;
const TOP_MARGIN = 72;
const LEFT_MARGIN = 64;
// Pages are paper whatever the theme, so the rules keep paper colors too.
const RULE = "#c4d3e6";
const MARGIN_RULE = "#e8b4b8";

/** Paper's ruling, drawn under a page's marks at the page's size in points. */
export function PaperBackground({ paper, width, height }: { paper: Paper; width: number; height: number }) {
  const pattern = `paper-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  return (
    <svg viewBox={`0 0 ${width} ${height}`} aria-hidden="true" data-paper={paper}
      className="tw:pointer-events-none tw:absolute tw:inset-0 tw:size-full">
      <PaperRuling paper={paper} width={width} height={height} pattern={pattern} />
    </svg>
  );
}

function PaperRuling({ paper, width, height, pattern }: { paper: Paper; width: number; height: number; pattern: string }) {
  if (paper === "blank") return null;
  if (paper === "lined") {
    const lines: number[] = [];
    for (let y = TOP_MARGIN; y < height - LINE_SPACING / 2; y += LINE_SPACING) lines.push(y);
    return (
      <>
        {lines.map((y) => <line key={y} x1={0} x2={width} y1={y} y2={y} stroke={RULE} strokeWidth={0.75} />)}
        <line x1={LEFT_MARGIN} x2={LEFT_MARGIN} y1={0} y2={height} stroke={MARGIN_RULE} strokeWidth={0.75} />
      </>
    );
  }
  return (
    <>
      <defs>
        <pattern id={pattern} width={GRID_SPACING} height={GRID_SPACING} patternUnits="userSpaceOnUse">
          {paper === "grid" ? (
            <path d={`M ${GRID_SPACING} 0 L 0 0 0 ${GRID_SPACING}`} fill="none" stroke={RULE} strokeWidth={0.6} />
          ) : (
            <circle cx={GRID_SPACING / 2} cy={GRID_SPACING / 2} r={0.9} fill={RULE} />
          )}
        </pattern>
      </defs>
      <rect width={width} height={height} fill={`url(#${pattern})`} />
    </>
  );
}

/** Space kept around a board's marks in its thumbnail, in points. */
const BOARD_THUMBNAIL_MARGIN = 36;

/** The part of a page a thumbnail shows: all of a page, the marks of a board (or its middle when empty). */
function thumbnailView(page: NotebookPage | null, items: readonly NotebookItem[]): string {
  const width = page?.width ?? 612;
  const height = page?.height ?? 792;
  if (page?.layout !== "board") return `0 0 ${width} ${height}`;
  let left = Infinity;
  let top = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (const item of items) {
    // A thumbnail's text is one line, so one line's height is enough.
    const bounds = shapeBounds(item.shape, item.shape.kind === "text" ? item.shape.fontSize * 1.4 : 0);
    left = Math.min(left, bounds.left);
    top = Math.min(top, bounds.top);
    right = Math.max(right, bounds.right);
    bottom = Math.max(bottom, bounds.bottom);
  }
  if (left > right) return `${(width - 612) / 2} ${(height - 792) / 2} 612 792`;
  // Kept to a page's shape, so a wide map is not squeezed into a sliver.
  const viewWidth = Math.max(right - left, (bottom - top) * 612 / 792) + BOARD_THUMBNAIL_MARGIN * 2;
  const viewHeight = Math.max(bottom - top, (right - left) * 792 / 612) + BOARD_THUMBNAIL_MARGIN * 2;
  return `${(left + right - viewWidth) / 2} ${(top + bottom - viewHeight) / 2} ${viewWidth} ${viewHeight}`;
}

/** A notebook's first page, small: its paper and the marks drawn on it; a board shows where its marks are. */
export function NotebookThumbnail({ page, items }: { page: NotebookPage | null; items: readonly NotebookItem[] }) {
  const pattern = `thumb-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const width = page?.width ?? 612;
  const height = page?.height ?? 792;
  return (
    <svg viewBox={thumbnailView(page, items)} aria-hidden="true" preserveAspectRatio="xMidYMin meet"
      className="tw:block tw:size-full tw:bg-[var(--ostudy-color-neutral-0)]">
      <rect width={width} height={height} fill="#ffffff" />
      {page ? <PaperRuling paper={page.paper} width={width} height={height} pattern={pattern} /> : null}
      {items.map((item) => item.shape.kind === "text" ? (
        <text key={item.id} x={item.shape.x + 4} y={item.shape.y + item.shape.fontSize} fontSize={item.shape.fontSize}
          fill={item.shape.sticky || item.shape.link?.button ? "#1f2328" : item.shape.color}>
          {item.shape.text.split("\n")[0]?.slice(0, 80)}
        </text>
      ) : (
        <g key={item.id} opacity={item.shape.kind === "highlight" ? 0.75 : undefined}>
          <ShapeGraphic shape={item.shape} />
        </g>
      ))}
    </svg>
  );
}

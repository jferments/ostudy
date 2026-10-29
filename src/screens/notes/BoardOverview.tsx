import { useEffect, useMemo, useState, type PointerEvent, type RefObject } from "react";

import { shapeBounds, type Annotation, type Bounds } from "../documents/annotator/annotationModel";
import type { PageRect } from "../documents/annotator/useAnnotatorViewport";

/** Space kept around a board's marks when it is fitted or overviewed, in points. */
export const BOARD_MARGIN = 48;
const OVERVIEW_WIDTH = 176;
const OVERVIEW_HEIGHT = 120;

/** Where a board's marks reach, or null when it has none. */
export function marksBounds(annotations: readonly Annotation[], textHeight: (annotation: Annotation) => number): Bounds | null {
  let bounds: Bounds | null = null;
  for (const annotation of annotations) {
    const next = shapeBounds(annotation.shape, textHeight(annotation));
    bounds = bounds === null ? next : {
      left: Math.min(bounds.left, next.left),
      top: Math.min(bounds.top, next.top),
      right: Math.max(bounds.right, next.right),
      bottom: Math.max(bounds.bottom, next.bottom),
    };
  }
  return bounds;
}

/** Bounds as a region with `margin` around them, kept on a page of `width` by `height`. */
export function regionAround(bounds: Bounds, margin: number, width: number, height: number): PageRect {
  const x = Math.max(0, bounds.left - margin);
  const y = Math.max(0, bounds.top - margin);
  return {
    x,
    y,
    width: Math.min(width, bounds.right + margin) - x,
    height: Math.min(height, bounds.bottom + margin) - y,
  };
}

interface BoardOverviewProps {
  pageNumber: number;
  width: number;
  height: number;
  annotations: readonly Annotation[];
  textHeight: (annotation: Annotation) => number;
  /** The zoom, so the outline of the view follows it. */
  scale: number;
  viewportRef: RefObject<HTMLDivElement | null>;
  visibleRegion: (pageNumber: number) => PageRect | null;
  /** Brings a spot of the board to the middle of the view. */
  onCenter: (point: { x: number; y: number }) => void;
}

/**
 * A small map of a board in the corner of the view: its marks as blocks and
 * the part in view as an outline. Pressing or dragging on it moves the view
 * there. Fit all does the same from the keyboard.
 */
export function BoardOverview({
  pageNumber, width, height, annotations, textHeight, scale, viewportRef, visibleRegion, onCenter,
}: BoardOverviewProps) {
  const [view, setView] = useState<PageRect | null>(null);
  useEffect(() => {
    const element = viewportRef.current;
    if (!element) return undefined;
    let frame: number | null = null;
    const update = () => {
      frame = null;
      setView(visibleRegion(pageNumber));
    };
    const schedule = () => { frame ??= requestAnimationFrame(update); };
    update();
    element.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule);
    return () => {
      if (frame !== null) cancelAnimationFrame(frame);
      element.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
    };
  }, [pageNumber, scale, viewportRef, visibleRegion]);

  const marks = useMemo(
    () => annotations.map((annotation) => ({ id: annotation.id, bounds: shapeBounds(annotation.shape, textHeight(annotation)) })),
    [annotations, textHeight],
  );
  const frame = useMemo(() => {
    let bounds = marksBounds(annotations, textHeight);
    if (view) {
      const seen = { left: view.x, top: view.y, right: view.x + view.width, bottom: view.y + view.height };
      bounds = bounds === null ? seen : {
        left: Math.min(bounds.left, seen.left),
        top: Math.min(bounds.top, seen.top),
        right: Math.max(bounds.right, seen.right),
        bottom: Math.max(bounds.bottom, seen.bottom),
      };
    }
    if (bounds === null) return null;
    const region = regionAround(bounds, BOARD_MARGIN * 4, width, height);
    // Widened to the overview's own shape, so nothing is stretched.
    const aspect = OVERVIEW_WIDTH / OVERVIEW_HEIGHT;
    const wide = Math.max(region.width, region.height * aspect);
    const tall = wide / aspect;
    return {
      x: region.x + region.width / 2 - wide / 2,
      y: region.y + region.height / 2 - tall / 2,
      width: wide,
      height: tall,
    };
  }, [annotations, height, textHeight, view, width]);

  if (!frame) return null;
  const move = (event: PointerEvent<SVGSVGElement>) => {
    if (event.buttons === 0 && event.type === "pointermove") return;
    const rect = event.currentTarget.getBoundingClientRect();
    onCenter({
      x: Math.min(width, Math.max(0, frame.x + (event.clientX - rect.left) / rect.width * frame.width)),
      y: Math.min(height, Math.max(0, frame.y + (event.clientY - rect.top) / rect.height * frame.height)),
    });
  };
  return (
    <svg
      role="img"
      aria-label="Board overview"
      data-board-overview="true"
      viewBox={`${frame.x} ${frame.y} ${frame.width} ${frame.height}`}
      width={OVERVIEW_WIDTH}
      height={OVERVIEW_HEIGHT}
      className="tw:absolute tw:end-3 tw:bottom-3 tw:cursor-pointer tw:touch-none tw:rounded-md tw:border tw:border-solid tw:border-border tw:bg-background/90 tw:shadow-md"
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture?.(event.pointerId);
        move(event);
      }}
      onPointerMove={move}
    >
      <rect x={0} y={0} width={width} height={height} className="tw:fill-[var(--ostudy-color-neutral-0)]" />
      {marks.map(({ id, bounds }) => (
        <rect key={id} x={bounds.left} y={bounds.top} width={Math.max(1, bounds.right - bounds.left)}
          height={Math.max(1, bounds.bottom - bounds.top)} className="tw:fill-muted-foreground/50" />
      ))}
      {view ? (
        <rect x={view.x} y={view.y} width={view.width} height={view.height} vectorEffect="non-scaling-stroke"
          className="tw:fill-primary/10 tw:stroke-primary" strokeWidth={1.5} />
      ) : null}
    </svg>
  );
}

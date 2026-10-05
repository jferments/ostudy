import {
  Fragment,
  useCallback,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
} from "react";
import { cn } from "../internal/cn";

const KEYBOARD_STEP = 16;
const KEYBOARD_LARGE_STEP = 64;

export interface ResizablePane {
  id: string;
  /** Names the pane for its resize handle ("Resize {label}"). */
  label: string;
  content: ReactNode;
  /**
   * Width in px of a fixed pane. Exactly one pane leaves it out and takes the
   * remaining width.
   */
  defaultSize?: number;
  minSize: number;
  maxSize?: number;
  className?: string;
}

export interface ResizablePanesProps {
  panes: readonly ResizablePane[];
  /** Saved widths by pane id; panes without one use their default. */
  sizes?: Readonly<Record<string, number>>;
  /** Called once a drag or key press finishes, with every fixed pane's width. */
  onSizesChange?: (sizes: Record<string, number>) => void;
  className?: string;
}

/**
 * Side-by-side panes with draggable, keyboard-operable handles between them.
 * Fixed panes keep a width in px; the one flexible pane fills the rest and
 * never shrinks below its minimum. Use it on wide layouts only: compact
 * screens show one pane at a time instead.
 */
export function ResizablePanes({ panes, sizes, onSizesChange, className }: ResizablePanesProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [draft, setDraft] = useState<Record<string, number> | null>(null);
  const flexible = panes.find((pane) => pane.defaultSize === undefined);

  const widthOf = useCallback((pane: ResizablePane, source: Readonly<Record<string, number>> | null | undefined) => (
    clamp(source?.[pane.id] ?? sizes?.[pane.id] ?? pane.defaultSize ?? pane.minSize, pane.minSize, pane.maxSize)
  ), [sizes]);

  const currentSizes = useCallback((source: Readonly<Record<string, number>> | null) => {
    const result: Record<string, number> = {};
    for (const pane of panes) {
      if (pane.defaultSize !== undefined) result[pane.id] = widthOf(pane, source);
    }
    return result;
  }, [panes, widthOf]);

  /** The widest a fixed pane can grow while the flexible pane keeps its minimum. */
  const maximumFor = useCallback((pane: ResizablePane, source: Readonly<Record<string, number>>) => {
    const containerWidth = containerRef.current?.getBoundingClientRect().width ?? 0;
    if (containerWidth <= 0 || !flexible) return pane.maxSize;
    const others = panes
      .filter((other) => other !== pane && other.defaultSize !== undefined)
      .reduce((total, other) => total + (source[other.id] ?? 0), 0);
    const room = containerWidth - others - flexible.minSize;
    return Math.max(pane.minSize, pane.maxSize === undefined ? room : Math.min(pane.maxSize, room));
  }, [flexible, panes]);

  const resize = useCallback((pane: ResizablePane, width: number, commit: boolean) => {
    const base = currentSizes(draft);
    const next = { ...base, [pane.id]: clamp(Math.round(width), pane.minSize, maximumFor(pane, base)) };
    if (commit) {
      setDraft(null);
      onSizesChange?.(next);
    } else {
      setDraft(next);
    }
    return next;
  }, [currentSizes, draft, maximumFor, onSizesChange]);

  const shown = currentSizes(draft);
  return (
    <div ref={containerRef} className={cn("tw:flex tw:h-full tw:min-h-0 tw:w-full tw:min-w-0", className)}>
      {panes.map((pane, index) => {
        const next = panes[index + 1];
        // A handle resizes the fixed pane beside it: the left one if fixed, else the right one.
        const target = pane.defaultSize !== undefined ? pane : next?.defaultSize !== undefined ? next : undefined;
        const direction = target === pane ? 1 : -1;
        return (
          <Fragment key={pane.id}>
            <div
              id={`pane-${pane.id}`}
              data-pane={pane.id}
              className={cn(
                "tw:flex tw:h-full tw:min-h-0 tw:flex-col tw:overflow-hidden",
                pane.defaultSize === undefined ? "tw:min-w-0 tw:flex-1" : "tw:shrink-0",
                pane.className,
              )}
              style={pane.defaultSize === undefined ? { minWidth: pane.minSize } : { width: shown[pane.id] }}
            >
              {pane.content}
            </div>
            {next && target ? (
              <PaneHandle
                pane={target}
                width={shown[target.id] ?? target.minSize}
                maximum={maximumFor(target, shown) ?? target.maxSize}
                direction={direction}
                onResize={(width, commit) => resize(target, width, commit)}
              />
            ) : null}
          </Fragment>
        );
      })}
    </div>
  );
}

interface PaneHandleProps {
  pane: ResizablePane;
  width: number;
  maximum: number | undefined;
  /** 1 when dragging right widens the pane, -1 when it narrows it. */
  direction: 1 | -1;
  onResize: (width: number, commit: boolean) => Record<string, number>;
}

function PaneHandle({ pane, width, maximum, direction, onResize }: PaneHandleProps) {
  const drag = useRef<{ pointerId: number; startX: number; startWidth: number; last: number } | null>(null);

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture?.(event.pointerId);
    drag.current = { pointerId: event.pointerId, startX: event.clientX, startWidth: width, last: width };
  };
  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const active = drag.current;
    if (!active || active.pointerId !== event.pointerId) return;
    active.last = active.startWidth + (event.clientX - active.startX) * direction;
    onResize(active.last, false);
  };
  const finish = (event: PointerEvent<HTMLDivElement>) => {
    const active = drag.current;
    if (!active || active.pointerId !== event.pointerId) return;
    drag.current = null;
    onResize(active.last, true);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.shiftKey ? KEYBOARD_LARGE_STEP : KEYBOARD_STEP;
    let next: number | null = null;
    if (event.key === "ArrowLeft") next = width - step * direction;
    else if (event.key === "ArrowRight") next = width + step * direction;
    else if (event.key === "Home") next = pane.minSize;
    else if (event.key === "End" && maximum !== undefined) next = maximum;
    if (next === null) return;
    event.preventDefault();
    onResize(next, true);
  };

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={`Resize ${pane.label}`}
      aria-controls={`pane-${pane.id}`}
      aria-valuenow={Math.round(width)}
      aria-valuemin={pane.minSize}
      aria-valuemax={maximum}
      tabIndex={0}
      data-pane-handle={pane.id}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={finish}
      onPointerCancel={finish}
      onDoubleClick={() => onResize(pane.defaultSize ?? pane.minSize, true)}
      onKeyDown={onKeyDown}
      className={cn(
        "tw:group tw:relative tw:z-(--ostudy-layer-raised) tw:-mx-1 tw:flex tw:w-2 tw:shrink-0 tw:cursor-col-resize tw:touch-none tw:justify-center tw:outline-none",
        "tw:coarse:-mx-2 tw:coarse:w-4",
      )}
    >
      <span
        aria-hidden="true"
        className="tw:h-full tw:w-px tw:bg-border tw:transition-colors tw:group-hover:bg-border-strong tw:group-focus-visible:w-0.5 tw:group-focus-visible:bg-primary tw:reduce-motion:transition-none"
      />
    </div>
  );
}

function clamp(value: number, minimum: number, maximum: number | undefined) {
  return Math.max(minimum, maximum === undefined ? value : Math.min(maximum, value));
}

import {
  measureElement as measureVirtualElement,
  observeElementRect,
  useVirtualizer,
  type Virtualizer,
} from "@tanstack/react-virtual";
import { useCallback, useEffect, useImperativeHandle, useRef, type ReactNode, type Ref } from "react";
import { cn } from "../internal/cn";

/** Fallback viewport height while a list has no layout (before mount, hidden, or under tests). */
const UNMEASURED_VIEWPORT_HEIGHT = 1200;

export interface VirtualListHandle {
  /** Scrolls the item into view without moving focus. */
  scrollToIndex: (index: number) => void;
}

export interface VirtualListProps<T> {
  items: readonly T[];
  itemKey: (item: T, index: number) => string;
  renderItem: (item: T, index: number) => ReactNode;
  /** Expected row height in px; rows are measured once rendered, so they may wrap. */
  estimateItemSize: number;
  /** Accessible name of the list. */
  label: string;
  /** Rows rendered beyond the visible ones in each direction. */
  overscan?: number;
  /** Called when a row within `endThreshold` rows of the end is rendered, to load the next page. */
  onEndReached?: () => void;
  endThreshold?: number;
  /** Shown after the last row inside the scroll area, such as a loading line. */
  footer?: ReactNode;
  className?: string;
  ref?: Ref<VirtualListHandle>;
}

/**
 * A scrolling list that renders only the rows near the viewport, for lists
 * that can run to thousands of rows. The list owns its scroll area, so give it
 * a bounded height (a flex child with `min-h-0`, or an explicit height).
 */
export function VirtualList<T>({
  items,
  itemKey,
  renderItem,
  estimateItemSize,
  label,
  overscan = 8,
  onEndReached,
  endThreshold = 10,
  footer,
  className,
  ref,
}: VirtualListProps<T>) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const getItemKey = useCallback((index: number) => itemKey(items[index]!, index), [itemKey, items]);
  const virtualizer = useVirtualizer<HTMLDivElement, HTMLDivElement>({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => estimateItemSize,
    getItemKey,
    overscan,
    observeElementRect: observeRectWithFallback,
    measureElement: (element, entry, instance) => {
      const size = measureVirtualElement(element, entry, instance);
      return size > 0 ? size : estimateItemSize;
    },
  });
  useImperativeHandle(ref, () => ({
    scrollToIndex: (index: number) => virtualizer.scrollToIndex(index, { align: "auto" }),
  }), [virtualizer]);

  const virtualItems = virtualizer.getVirtualItems();
  const lastRendered = virtualItems.at(-1)?.index ?? -1;
  const endReached = items.length > 0 && lastRendered >= items.length - 1 - endThreshold;
  const onEndReachedRef = useRef(onEndReached);
  useEffect(() => { onEndReachedRef.current = onEndReached; });
  useEffect(() => {
    if (endReached) onEndReachedRef.current?.();
  }, [endReached, items.length]);

  return (
    <div
      ref={scrollRef}
      data-virtual-list="true"
      className={cn("tw:relative tw:min-h-0 tw:overflow-y-auto tw:overscroll-contain", className)}
    >
      <div role="list" aria-label={label} className="tw:relative tw:w-full" style={{ height: virtualizer.getTotalSize() }}>
        {virtualItems.map((virtualItem) => (
          <div
            key={virtualItem.key}
            role="listitem"
            data-index={virtualItem.index}
            aria-setsize={items.length}
            aria-posinset={virtualItem.index + 1}
            ref={virtualizer.measureElement}
            className="tw:absolute tw:top-0 tw:left-0 tw:w-full"
            style={{ transform: `translateY(${virtualItem.start}px)` }}
          >
            {renderItem(items[virtualItem.index]!, virtualItem.index)}
          </div>
        ))}
      </div>
      {footer}
    </div>
  );
}

/** Like the default observer, but a list with no layout yet still renders a viewport's worth of rows. */
function observeRectWithFallback(
  instance: Virtualizer<HTMLDivElement, HTMLDivElement>,
  callback: (rect: { width: number; height: number }) => void,
) {
  return observeElementRect(instance, (rect) => {
    callback(rect.height > 0 ? rect : { width: rect.width, height: UNMEASURED_VIEWPORT_HEIGHT });
  });
}

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type RefObject } from "react";

import type { AnnotatedPage } from "../../../ipc/documentAnnotations";
import { usePlatformCapabilities } from "../../../platform/PlatformProvider";
import type { ZoomAnchor } from "./gestures";
import type { PageRegion } from "./pageTiles";

/** CSS pixels per PDF point: a fifth of print size up to three times it. */
export const MIN_SCALE = 0.25;
export const MAX_SCALE = 4;
/** Print size: 96 CSS pixels to the inch, 72 points to the inch. */
export const ACTUAL_SIZE_SCALE = 96 / 72;
/**
 * Fitting the window stops at 125% of print size, where a page image (drawn at
 * 120 dpi on import) shows at its own resolution. A wide window keeps the page
 * sharp and centered rather than blowing it up; zooming in still goes further.
 */
const MAX_FIT_SCALE = 120 / 72;
const ZOOM_STEP = 1.25;
const MIN_VIEWPORT_HEIGHT = 240;
/** Pages this far beyond the visible area (in viewport heights) load their images. */
const NEAR_MARGIN = "150% 0px";

/** How long the view rests after scrolling or zooming before sharp tiles are drawn. */
const SETTLE_MS = 150;
/** Tiles are drawn this far (CSS pixels) beyond the visible area, ready for a short scroll. */
const REGION_MARGIN = 256;

type Zoom = { mode: "fit" } | { mode: "manual"; scale: number };

/**
 * Where the view came to rest: its zoom, the screen's density, and the part
 * of each page in view (with a margin), in points.
 */
export interface SettledView {
  scale: number;
  pixelRatio: number;
  regions: ReadonlyMap<number, PageRegion>;
}

function clampScale(scale: number): number {
  return Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale));
}

/**
 * The scrolling page area: its height (the rest of the screen, above an
 * Android keyboard), its zoom (automatic, fitting the window up to 125%,
 * until the learner zooms), which pages are near enough to draw, what is in
 * view once the view rests, and scrolling that keeps a chosen spot of a page
 * in place while the zoom changes.
 */
export function useAnnotatorViewport(
  pages: readonly AnnotatedPage[],
  viewportRef: RefObject<HTMLDivElement | null>,
  contentRef: RefObject<HTMLDivElement | null>,
) {
  const capabilities = usePlatformCapabilities();
  const imeBottom = capabilities.androidInsets?.imeVisible ? Math.max(0, capabilities.androidInsets.imeBottom) : 0;
  const [height, setHeight] = useState(MIN_VIEWPORT_HEIGHT * 2);
  const [fitWidth, setFitWidth] = useState(0);
  const [zoom, setZoom] = useState<Zoom>({ mode: "fit" });
  const [near, setNear] = useState<ReadonlySet<number>>(() => new Set(pages.slice(0, 3).map((page) => page.pageNumber)));
  const [settled, setSettled] = useState<SettledView | null>(null);
  const pendingAnchor = useRef<{ anchor: ZoomAnchor; clientX: number; clientY: number } | null>(null);
  const fling = useRef<number | null>(null);

  const widest = useMemo(() => Math.max(1, ...pages.map((page) => page.width)), [pages]);
  const fitScale = fitWidth > 0 ? clampScale(Math.min(MAX_FIT_SCALE, fitWidth / widest)) : 1;
  const scale = zoom.mode === "fit" ? fitScale : zoom.scale;
  const scaleRef = useRef(scale);
  scaleRef.current = scale;
  const fitScaleRef = useRef(fitScale);
  fitScaleRef.current = fitScale;

  // The page area fills the rest of the app's scroll area, so the toolbar stays put.
  useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return undefined;
    const main = viewport.closest<HTMLElement>("[data-slot='sidebar-inset']");
    if (main) main.scrollTop = 0;
    const content = contentRef.current;
    const update = () => {
      const area = main?.getBoundingClientRect() ?? { top: 0, bottom: window.innerHeight };
      const top = viewport.getBoundingClientRect().top - area.top + (main?.scrollTop ?? 0);
      const bottom = Math.min(area.bottom, window.innerHeight - imeBottom) - area.top;
      setHeight(Math.max(MIN_VIEWPORT_HEIGHT, Math.floor(bottom - top)));
      const style = content ? getComputedStyle(content) : null;
      const padding = style ? (parseFloat(style.paddingLeft) || 0) + (parseFloat(style.paddingRight) || 0) : 0;
      setFitWidth(Math.max(0, viewport.clientWidth - padding));
    };
    update();
    window.addEventListener("resize", update);
    if (typeof ResizeObserver === "undefined") return () => window.removeEventListener("resize", update);
    const observer = new ResizeObserver(update);
    observer.observe(viewport);
    // The toolbar above the pages can wrap to another row.
    if (viewport.parentElement) observer.observe(viewport.parentElement);
    if (main) observer.observe(main);
    return () => {
      window.removeEventListener("resize", update);
      observer.disconnect();
    };
  }, [contentRef, imeBottom, viewportRef]);

  // Pages near the visible area show their images and annotations; the rest keep only their size.
  useEffect(() => {
    const viewport = viewportRef.current;
    const content = contentRef.current;
    if (!viewport || !content) return undefined;
    if (typeof IntersectionObserver === "undefined") {
      setNear(new Set(pages.map((page) => page.pageNumber)));
      return undefined;
    }
    const observer = new IntersectionObserver((entries) => {
      setNear((current) => {
        const next = new Set(current);
        for (const entry of entries) {
          const pageNumber = Number((entry.target as HTMLElement).dataset.annotatorPage);
          if (entry.isIntersecting) next.add(pageNumber);
          else next.delete(pageNumber);
        }
        return next.size === current.size && [...next].every((page) => current.has(page)) ? current : next;
      });
    }, { root: viewport, rootMargin: NEAR_MARGIN });
    content.querySelectorAll<HTMLElement>("[data-annotator-page]").forEach((page) => observer.observe(page));
    return () => observer.disconnect();
  }, [contentRef, pages, viewportRef]);

  // Once scrolling and zooming rest, record what is in view, for sharp tiles.
  useEffect(() => {
    const viewport = viewportRef.current;
    const content = contentRef.current;
    if (!viewport || !content) return undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const measure = () => {
      timer = undefined;
      const view = viewport.getBoundingClientRect();
      const current = scaleRef.current;
      const regions = new Map<number, PageRegion>();
      for (const element of content.querySelectorAll<HTMLElement>("[data-annotator-page]")) {
        const rect = element.getBoundingClientRect();
        const left = Math.max(rect.left, view.left - REGION_MARGIN);
        const right = Math.min(rect.right, view.right + REGION_MARGIN);
        const top = Math.max(rect.top, view.top - REGION_MARGIN);
        const bottom = Math.min(rect.bottom, view.bottom + REGION_MARGIN);
        if (right <= left || bottom <= top) continue;
        regions.set(Number(element.dataset.annotatorPage), {
          x: (left - rect.left) / current,
          y: (top - rect.top) / current,
          width: (right - left) / current,
          height: (bottom - top) / current,
        });
      }
      setSettled({ scale: current, pixelRatio: window.devicePixelRatio || 1, regions });
    };
    const schedule = () => {
      if (timer !== undefined) clearTimeout(timer);
      timer = setTimeout(measure, SETTLE_MS);
    };
    schedule();
    viewport.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule);
    return () => {
      if (timer !== undefined) clearTimeout(timer);
      viewport.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
    };
  }, [contentRef, height, pages, scale, viewportRef]);

  const pageElement = useCallback((pageNumber: number) => (
    contentRef.current?.querySelector<HTMLElement>(`[data-annotator-page="${pageNumber}"]`) ?? null
  ), [contentRef]);

  /** The page spot under a client position, measured from the nearest page. */
  const anchorAt = useCallback((clientX: number, clientY: number): ZoomAnchor | null => {
    const content = contentRef.current;
    if (!content) return null;
    let best: { anchor: ZoomAnchor; distance: number } | null = null;
    for (const element of content.querySelectorAll<HTMLElement>("[data-annotator-page]")) {
      const rect = element.getBoundingClientRect();
      const distance = clientY < rect.top ? rect.top - clientY : clientY > rect.bottom ? clientY - rect.bottom : 0;
      if (best && distance >= best.distance) continue;
      best = {
        distance,
        anchor: {
          pageNumber: Number(element.dataset.annotatorPage),
          point: { x: (clientX - rect.left) / scaleRef.current, y: (clientY - rect.top) / scaleRef.current },
        },
      };
      if (distance === 0) break;
    }
    return best?.anchor ?? null;
  }, [contentRef]);

  const viewportCenter = useCallback(() => {
    const rect = viewportRef.current?.getBoundingClientRect();
    return rect ? { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 } : null;
  }, [viewportRef]);

  /** Zooms keeping `anchor` under the client position (the center of the view by default). */
  /** Scrolls so a page spot sits under a client position at `scale`. */
  const placeAnchor = useCallback((anchor: ZoomAnchor, clientX: number, clientY: number, scale: number) => {
    const viewport = viewportRef.current;
    const element = pageElement(anchor.pageNumber);
    if (!viewport || !element) return;
    const rect = viewport.getBoundingClientRect();
    viewport.scrollLeft = element.offsetLeft + anchor.point.x * scale - (clientX - rect.left - viewport.clientLeft);
    viewport.scrollTop = element.offsetTop + anchor.point.y * scale - (clientY - rect.top - viewport.clientTop);
  }, [pageElement, viewportRef]);

  const zoomTo = useCallback((next: number | "fit", anchor?: ZoomAnchor | null, clientX?: number, clientY?: number) => {
    const target = next === "fit" ? fitScaleRef.current : clampScale(next);
    setZoom(next === "fit" ? { mode: "fit" } : { mode: "manual", scale: target });
    const center = viewportCenter();
    const x = clientX ?? center?.x;
    const y = clientY ?? center?.y;
    const kept = anchor ?? (x === undefined || y === undefined ? null : anchorAt(x, y));
    if (!kept || x === undefined || y === undefined) return;
    if (target === scaleRef.current) {
      // Fingers at the zoom limit still pan.
      if (anchor) placeAnchor(kept, x, y, target);
      return;
    }
    // Applied once the pages have their new size.
    pendingAnchor.current = { anchor: kept, clientX: x, clientY: y };
    scaleRef.current = target;
  }, [anchorAt, placeAnchor, viewportCenter]);

  const zoomBy = useCallback((factor: number) => zoomTo(scaleRef.current * factor), [zoomTo]);

  useLayoutEffect(() => {
    const pending = pendingAnchor.current;
    pendingAnchor.current = null;
    if (pending) placeAnchor(pending.anchor, pending.clientX, pending.clientY, scale);
  }, [placeAnchor, scale]);

  const stopFling = useCallback(() => {
    if (fling.current !== null) cancelAnimationFrame(fling.current);
    fling.current = null;
  }, []);

  /** Keeps a flicked view moving and slowing, unless motion is reduced. */
  const startFling = useCallback((velocityX: number, velocityY: number) => {
    stopFling();
    const motion = document.documentElement.dataset.motion;
    if (motion === "reduced" || motion === "none") return;
    let vx = velocityX;
    let vy = velocityY;
    let last = performance.now();
    const step = (time: number) => {
      const viewport = viewportRef.current;
      const elapsed = Math.min(64, time - last);
      last = time;
      if (!viewport || Math.hypot(vx, vy) < 0.02) {
        fling.current = null;
        return;
      }
      viewport.scrollLeft += vx * elapsed;
      viewport.scrollTop += vy * elapsed;
      const decay = Math.pow(0.95, elapsed / 16);
      vx *= decay;
      vy *= decay;
      fling.current = requestAnimationFrame(step);
    };
    fling.current = requestAnimationFrame(step);
  }, [stopFling, viewportRef]);
  useEffect(() => stopFling, [stopFling]);

  // Ctrl or Cmd with the wheel (a trackpad pinch) zooms around the pointer instead of the window.
  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return undefined;
    const wheel = (event: WheelEvent) => {
      stopFling();
      if (!event.ctrlKey && !event.metaKey) return;
      event.preventDefault();
      const delta = event.deltaMode === 1 ? event.deltaY * 16 : event.deltaY;
      zoomTo(scaleRef.current * Math.exp(-delta * 0.002), anchorAt(event.clientX, event.clientY), event.clientX, event.clientY);
    };
    viewport.addEventListener("wheel", wheel, { passive: false });
    return () => viewport.removeEventListener("wheel", wheel);
  }, [anchorAt, stopFling, viewportRef, zoomTo]);

  return {
    height,
    scale,
    fit: zoom.mode === "fit",
    near,
    settled,
    zoomTo,
    zoomIn: () => zoomBy(ZOOM_STEP),
    zoomOut: () => zoomBy(1 / ZOOM_STEP),
    fitToWidth: () => zoomTo("fit"),
    startFling,
    stopFling,
  };
}

import { useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";

import type { ImageMask, NormalizedRect } from "../../ipc/questions";
import {
  FIT_VIEWPORT, clampViewport, imageTransform, moveRectangle, panViewport, sameRectangle,
  zoomToRegion, zoomViewport, type ImageViewport, type Point, type ResizeHandle, type Size,
} from "./geometry";
import { OcclusionGestures, type GestureResult, type OcclusionTool, type RegionHit, type RegionMutation } from "./gestures";

export interface OcclusionWorkspaceOptions {
  imageKey: string;
  imageSize: Size;
  masks: readonly ImageMask[];
  selectedMaskId: string | null;
  disabled?: boolean;
  navigationOnly?: boolean;
  onSelect: (id: string | null) => void;
  onMutation: (mutation: RegionMutation) => void;
  onDelete: (id: string) => void;
  onUndo: () => void;
  onRedo: () => void;
}

export function useOcclusionWorkspace(options: OcclusionWorkspaceOptions) {
  const svgRef = useRef<SVGSVGElement>(null);
  const latest = useRef(options);
  latest.current = options;
  const gestures = useMemo(() => new OcclusionGestures(), [options.imageKey]);
  const captures = useRef(new Set<number>());
  const [frame, setFrame] = useState<Size>({ width: 1, height: 1 });
  const frameRef = useRef(frame);
  const [viewport, setViewport] = useState<ImageViewport>(FIT_VIEWPORT);
  const viewportRef = useRef(viewport);
  const [tool, setToolState] = useState<OcclusionTool>(options.navigationOnly ? "pan" : "select");
  const [interaction, setInteraction] = useState<GestureResult>(gestures.snapshot());
  const [status, setStatus] = useState("");

  function releaseCaptures(): void {
    const svg = svgRef.current;
    for (const id of captures.current) {
      if (svg?.hasPointerCapture(id)) svg.releasePointerCapture(id);
    }
    captures.current.clear();
  }
  function cancel(): void {
    setInteraction(gestures.cancel());
    releaseCaptures();
  }
  function changeViewport(next: ImageViewport): void {
    viewportRef.current = next;
    setViewport(next);
  }
  function accept(result: GestureResult): void {
    setInteraction(result);
    if (result.viewport) changeViewport(result.viewport);
    if (result.select !== undefined) latest.current.onSelect(result.select);
    if (result.mutation && !latest.current.disabled && !latest.current.navigationOnly) {
      const mutation = result.mutation;
      if (mutation.kind === "change") {
        const current = latest.current.masks.find((mask) => mask.id === mutation.maskId);
        if (!current || !sameRectangle(current.rect, mutation.before)) {
          setStatus("The region changed during this gesture. No geometry was overwritten.");
          return;
        }
      }
      latest.current.onMutation(mutation);
      setStatus(mutation.kind === "create" ? "Region added." : "Region adjusted.");
    }
  }

  useLayoutEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;
    const resize = () => {
      const box = svg.getBoundingClientRect();
      const next = { width: Math.max(1, box.width), height: Math.max(1, box.height) };
      if (next.width === frameRef.current.width && next.height === frameRef.current.height) return;
      // Orientation and software-keyboard layout changes cancel only an unfinished
      // gesture. The image-relative viewport and the durable geometry remain.
      setInteraction(gestures.cancel());
      releaseCaptures();
      frameRef.current = next;
      setFrame(next);
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(svg);
    return () => observer.disconnect();
  }, [gestures]);

  useEffect(() => {
    changeViewport(FIT_VIEWPORT);
    setToolState(latest.current.navigationOnly ? "pan" : "select");
    setInteraction(gestures.cancel());
    setStatus("");
    const interrupted = () => { setInteraction(gestures.cancel()); releaseCaptures(); };
    const visibility = () => { if (document.hidden) interrupted(); };
    window.addEventListener("blur", interrupted);
    document.addEventListener("visibilitychange", visibility);
    return () => {
      gestures.cancel();
      releaseCaptures();
      window.removeEventListener("blur", interrupted);
      document.removeEventListener("visibilitychange", visibility);
    };
  }, [gestures]);

  useEffect(() => {
    if (options.disabled) cancel();
  }, [options.disabled, gestures]);

  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;
    const wheel = (event: WheelEvent) => {
      if (latest.current.disabled || gestures.isActive()) return;
      const current = viewportRef.current;
      const size = latest.current.imageSize;
      const box = svg.getBoundingClientRect();
      if (event.ctrlKey || event.metaKey) {
        event.preventDefault();
        const anchor = { x: event.clientX - box.left, y: event.clientY - box.top };
        changeViewport(zoomViewport(current, size, frameRef.current, current.zoom * Math.exp(-event.deltaY * 0.002), anchor));
      } else if (current.zoom > 1) {
        event.preventDefault();
        const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? frameRef.current.height : 1;
        changeViewport(panViewport(current, size, frameRef.current, { x: -event.deltaX * unit, y: -event.deltaY * unit }));
      }
    };
    // This local non-passive listener owns image gestures only, not page scrolling.
    svg.addEventListener("wheel", wheel, { passive: false });
    return () => svg.removeEventListener("wheel", wheel);
  }, [gestures]);

  function sample(event: PointerEvent<SVGSVGElement>) {
    const box = event.currentTarget.getBoundingClientRect();
    return { id: event.pointerId, x: event.clientX - box.left, y: event.clientY - box.top };
  }
  function pointerDown(event: PointerEvent<SVGSVGElement>): void {
    if (latest.current.disabled || (event.button !== 0 && event.button !== 1)) return;
    event.preventDefault();
    event.currentTarget.focus({ preventScroll: true });
    event.currentTarget.setPointerCapture(event.pointerId);
    captures.current.add(event.pointerId);
    const element = event.target instanceof Element ? event.target.closest("[data-mask-id]") : null;
    const maskId = element?.getAttribute("data-mask-id");
    const handle = element?.getAttribute("data-resize-handle");
    const hit: RegionHit | null = maskId ? { maskId } : null;
    if (hit && (handle === "nw" || handle === "ne" || handle === "sw" || handle === "se")) hit.handle = handle;
    accept(gestures.down(sample(event), hit, {
      image: latest.current.imageSize, frame: frameRef.current,
      viewport: clampViewport(viewportRef.current, latest.current.imageSize, frameRef.current),
      masks: latest.current.masks, tool: latest.current.navigationOnly || event.button === 1 ? "pan" : tool,
    }));
  }
  function pointerMove(event: PointerEvent<SVGSVGElement>): void {
    if (!gestures.owns(event.pointerId)) return;
    event.preventDefault();
    accept(gestures.move(sample(event)));
  }
  function pointerUp(event: PointerEvent<SVGSVGElement>): void {
    if (!gestures.owns(event.pointerId)) return;
    event.preventDefault();
    const result = gestures.up(sample(event));
    captures.current.delete(event.pointerId);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
    accept(result);
  }
  function pointerCancel(event: PointerEvent<SVGSVGElement>): void {
    if (gestures.owns(event.pointerId)) cancel();
  }
  function setTool(next: OcclusionTool): void { cancel(); setToolState(latest.current.navigationOnly ? "pan" : next); }
  function zoom(factor: number): void {
    cancel();
    const frame = frameRef.current;
    changeViewport(zoomViewport(viewportRef.current, latest.current.imageSize, frame, viewportRef.current.zoom * factor,
      { x: frame.width / 2, y: frame.height / 2 }));
  }
  function fit(): void { cancel(); changeViewport(FIT_VIEWPORT); }
  function focusRegion(rect: NormalizedRect): void {
    cancel();
    changeViewport(zoomToRegion(rect, latest.current.imageSize, frameRef.current));
  }
  function addRegion(): void {
    if (latest.current.disabled || latest.current.navigationOnly) return;
    cancel();
    const width = Math.min(1, Math.max(0.2, 1 / latest.current.imageSize.width));
    const height = Math.min(1, Math.max(0.15, 1 / latest.current.imageSize.height));
    const center = clampViewport(viewportRef.current, latest.current.imageSize, frameRef.current).center;
    const rect = moveRectangle({ x: 0, y: 0, width, height }, { x: center.x - width / 2, y: center.y - height / 2 });
    latest.current.onMutation({ kind: "create", rectangle: rect });
    setToolState("select");
  }
  function keyDown(event: KeyboardEvent<SVGSVGElement>): void {
    if (latest.current.disabled || event.nativeEvent.isComposing || event.altKey) return;
    const key = event.key.toLowerCase();
    if (key === "escape") { event.preventDefault(); cancel(); return; }
    if (event.ctrlKey || event.metaKey) {
      if (key === "z" && !latest.current.navigationOnly) {
        event.preventDefault(); cancel();
        if (event.shiftKey) latest.current.onRedo(); else latest.current.onUndo();
      }
      return;
    }
    if (key === "+" || key === "=") { event.preventDefault(); zoom(1.5); return; }
    if (key === "-") { event.preventDefault(); zoom(1 / 1.5); return; }
    if (latest.current.navigationOnly) {
      const step = event.shiftKey ? 80 : 24;
      const directions: Record<string, Point> = {
        arrowleft: { x: step, y: 0 }, arrowright: { x: -step, y: 0 },
        arrowup: { x: 0, y: step }, arrowdown: { x: 0, y: -step },
      };
      if (directions[key]) {
        event.preventDefault(); cancel();
        changeViewport(panViewport(viewportRef.current, latest.current.imageSize, frameRef.current, directions[key]));
      }
      return;
    }
    const selected = latest.current.masks.find((mask) => mask.id === latest.current.selectedMaskId);
    if (!selected) return;
    if (key === "delete" || key === "backspace") { event.preventDefault(); cancel(); latest.current.onDelete(selected.id); return; }
    const step = event.shiftKey ? 10 : 1;
    const deltas: Record<string, Point> = {
      arrowleft: { x: -step / latest.current.imageSize.width, y: 0 },
      arrowright: { x: step / latest.current.imageSize.width, y: 0 },
      arrowup: { x: 0, y: -step / latest.current.imageSize.height },
      arrowdown: { x: 0, y: step / latest.current.imageSize.height },
    };
    if (deltas[key]) {
      event.preventDefault(); cancel();
      latest.current.onMutation({ kind: "change", maskId: selected.id, before: selected.rect, after: moveRectangle(selected.rect, deltas[key]) });
    }
  }

  const effectiveViewport = clampViewport(viewport, options.imageSize, frame);
  return {
    svgRef, frame, viewport: effectiveViewport, transform: imageTransform(effectiveViewport, options.imageSize, frame),
    tool, setTool, interaction, status, selected: options.masks.find((mask) => mask.id === options.selectedMaskId),
    cancel, zoom, fit, focusRegion, addRegion,
    pointerDown, pointerMove, pointerUp, pointerCancel, keyDown,
  };
}

export const RESIZE_HANDLES: readonly ResizeHandle[] = ["nw", "ne", "sw", "se"];

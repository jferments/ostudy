import { useEffect, useState } from "react";

export const PHONE_MAX_WIDTH = 760;
export const TABLET_MAX_WIDTH = 1179;

export type LayoutMode = "phone" | "tablet" | "wide";
export type LayoutOrientation = "portrait" | "landscape" | "square";

export interface LayoutDiagnosticInput {
  width: number;
  height: number;
  devicePixelRatio: number;
  uiScalePercent: number;
  input: "coarse" | "fine" | "unknown";
}

export interface LayoutDiagnostics extends LayoutDiagnosticInput {
  mode: LayoutMode;
  orientation: LayoutOrientation;
  effectiveWidthAtScale: number;
}

export function resolveLayoutMode(width: number): LayoutMode {
  if (width <= PHONE_MAX_WIDTH) return "phone";
  if (width <= TABLET_MAX_WIDTH) return "tablet";
  return "wide";
}

export function createLayoutDiagnostics(input: LayoutDiagnosticInput): LayoutDiagnostics {
  const safeScale = Number.isFinite(input.uiScalePercent) && input.uiScalePercent > 0
    ? input.uiScalePercent
    : 100;
  return {
    ...input,
    uiScalePercent: safeScale,
    mode: resolveLayoutMode(input.width),
    orientation:
      input.width === input.height ? "square" : input.width > input.height ? "landscape" : "portrait",
    effectiveWidthAtScale: Math.round(input.width / (safeScale / 100)),
  };
}

export function readLayoutDiagnostics(): LayoutDiagnostics {
  if (typeof window === "undefined" || typeof document === "undefined") {
    return createLayoutDiagnostics({
      width: 1440,
      height: 900,
      devicePixelRatio: 1,
      uiScalePercent: 100,
      input: "unknown",
    });
  }
  const root = document.documentElement;
  const visualViewport = window.visualViewport;
  const width = Math.round(visualViewport?.width ?? window.innerWidth);
  const height = Math.round(visualViewport?.height ?? window.innerHeight);
  const scaleValue = Number.parseFloat(
    window.getComputedStyle(root).getPropertyValue("--ui-scale").trim(),
  );
  const input = root.dataset.input;
  return createLayoutDiagnostics({
    width,
    height,
    devicePixelRatio: window.devicePixelRatio || 1,
    uiScalePercent: Number.isFinite(scaleValue) ? Math.round(scaleValue * 100) : 100,
    input: input === "coarse" || input === "fine" ? input : "unknown",
  });
}

export function formatLayoutDiagnostics(value: LayoutDiagnostics): string {
  return [
    `mode=${value.mode}`,
    `viewport=${value.width}x${value.height}`,
    `orientation=${value.orientation}`,
    `uiScale=${value.uiScalePercent}%`,
    `effectiveWidth=${value.effectiveWidthAtScale}`,
    `devicePixelRatio=${value.devicePixelRatio}`,
    `input=${value.input}`,
  ].join(" · ");
}

function subscribeToViewport(update: () => void): () => void {
  if (typeof window === "undefined") return () => undefined;
  let frame: number | null = null;
  const schedule = () => {
    if (frame !== null) window.cancelAnimationFrame(frame);
    frame = window.requestAnimationFrame(() => {
      frame = null;
      update();
    });
  };
  window.addEventListener("resize", schedule);
  window.addEventListener("orientationchange", schedule);
  window.visualViewport?.addEventListener("resize", schedule);
  return () => {
    if (frame !== null) window.cancelAnimationFrame(frame);
    window.removeEventListener("resize", schedule);
    window.removeEventListener("orientationchange", schedule);
    window.visualViewport?.removeEventListener("resize", schedule);
  };
}

export function useLayoutMode(): LayoutMode {
  const [mode, setMode] = useState(() => readLayoutDiagnostics().mode);
  useEffect(() => subscribeToViewport(() => setMode(readLayoutDiagnostics().mode)), []);
  return mode;
}

export function useLayoutDiagnostics(): LayoutDiagnostics {
  const [diagnostics, setDiagnostics] = useState(readLayoutDiagnostics);
  useEffect(() => subscribeToViewport(() => setDiagnostics(readLayoutDiagnostics())), []);
  return diagnostics;
}

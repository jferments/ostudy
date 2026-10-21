import { useEffect, useRef, useState, useSyncExternalStore, type ReactNode } from "react";

import { cn } from "../../../ui/internal/cn";
import { ColorField } from "../../../ui/primitives/ColorField";
import { Switch } from "../../../ui/primitives/FormControls";
import { TextField } from "../../../ui/primitives/TextField";
import { Toolbar, ToolbarAction } from "../../../ui/primitives/Toolbar";
import {
  clampSize,
  sizeRange,
  STYLE_CHOICES,
  type ActiveStyle,
  type LineStyle,
  type ToolStyle,
} from "./annotationModel";

export type StylePatch = Partial<ToolStyle>;

const RECENT_KEY = "ostudy.annotator.recentColors";
const MAX_RECENT = 6;
const listeners = new Set<() => void>();
let recent: readonly string[] = readRecent();

function readRecent(): readonly string[] {
  try {
    const stored: unknown = JSON.parse(globalThis.localStorage?.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(stored) ? stored.filter((color): color is string => typeof color === "string" && /^#[0-9a-f]{6}$/.test(color)).slice(0, MAX_RECENT) : [];
  } catch {
    return [];
  }
}

/** Remembers a color picked from the full picker, newest first, so it is one tap away next time. */
export function rememberColor(color: string) {
  if (!/^#[0-9a-f]{6}$/.test(color)) return;
  recent = [color, ...recent.filter((known) => known !== color)].slice(0, MAX_RECENT);
  try {
    globalThis.localStorage?.setItem(RECENT_KEY, JSON.stringify(recent));
  } catch {
    // The list still lasts for this session.
  }
  for (const listener of listeners) listener();
}

function useRecentColors(): readonly string[] {
  return useSyncExternalStore(
    (listener) => { listeners.add(listener); return () => listeners.delete(listener); },
    () => recent,
  );
}

/**
 * The browser's full color picker. A picker reports every color it passes
 * through while it is dragged; only the one the learner settles on is applied,
 * so a drag is one change to undo.
 */
function PickedColor({ label, value, onPick, disabled }: {
  label: string; value: string; onPick: (color: string) => void; disabled?: boolean;
}) {
  const ref = useRef<HTMLInputElement>(null);
  const [shown, setShown] = useState(value);
  const pick = useRef(onPick);
  pick.current = onPick;
  useEffect(() => setShown(value), [value]);
  useEffect(() => {
    const input = ref.current;
    if (!input) return;
    const settle = () => {
      const color = input.value.toLowerCase();
      rememberColor(color);
      pick.current(color);
    };
    input.addEventListener("change", settle);
    return () => input.removeEventListener("change", settle);
  }, []);
  return (
    <ColorField ref={ref} aria-label={label} title={label} value={shown} disabled={disabled}
      onChange={(event) => setShown(event.target.value)} className="tw:h-8 tw:min-h-8 tw:w-10 tw:coarse:min-h-touch" />
  );
}

function Swatch({ color }: { color: string }) {
  return (
    <span aria-hidden="true" className="tw:size-5 tw:rounded-full tw:ring-1 tw:ring-foreground/30"
      style={{ backgroundColor: color }} />
  );
}

/** The family's swatches, then colors picked recently, then the full picker. */
export function ColorChoices({ style, onStyle, picker = true }: {
  style: ActiveStyle; onStyle: (patch: StylePatch) => void; picker?: boolean;
}) {
  const swatches = STYLE_CHOICES[style.family].swatches;
  const recentColors = useRecentColors().filter((color) => !swatches.some((swatch) => swatch.color === color));
  return (
    <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-1">
      <Toolbar label="Color" className="tw:flex-wrap tw:gap-1">
        {swatches.map((swatch) => (
          <ToolbarAction key={swatch.color} label={swatch.label} pressed={style.style.color === swatch.color}
            onClick={() => onStyle({ color: swatch.color })}>
            <Swatch color={swatch.color} />
          </ToolbarAction>
        ))}
        {recentColors.map((color) => (
          <ToolbarAction key={color} label={`Recent color ${color}`} pressed={style.style.color === color}
            onClick={() => onStyle({ color })}>
            <Swatch color={color} />
          </ToolbarAction>
        ))}
      </Toolbar>
      {picker ? <PickedColor label="Any color" value={style.style.color} onPick={(color) => onStyle({ color })} /> : null}
    </div>
  );
}

/** A size typed in points, applied when the field is left or Enter is pressed. */
export function SizeField({ style, onStyle }: { style: ActiveStyle; onStyle: (patch: StylePatch) => void }) {
  const [low, high] = sizeRange(style.family);
  const [text, setText] = useState(String(style.style.size));
  useEffect(() => setText(String(style.style.size)), [style.style.size]);
  const apply = () => {
    const value = Number.parseFloat(text);
    if (!Number.isFinite(value)) {
      setText(String(style.style.size));
      return;
    }
    const size = clampSize(style.family, value);
    setText(String(size));
    if (size !== style.style.size) onStyle({ size });
  };
  const textual = style.family === "text" || style.family === "sticky";
  return (
    <span className="tw:flex tw:items-center tw:gap-1 tw:text-label tw:text-muted-foreground">
      <TextField type="number" inputMode="decimal" min={low} max={high} step={textual ? 1 : 0.25} value={text}
        aria-label={textual ? "Text size in points" : "Line width in points"}
        onChange={(event) => setText(event.target.value)} onBlur={apply}
        onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); apply(); } }}
        className="tw:h-8 tw:min-h-8 tw:w-16 tw:px-2 tw:tabular-nums tw:coarse:min-h-touch" />
      <span aria-hidden="true">pt</span>
    </span>
  );
}

const LINE_STYLES: ReadonlyArray<{ value: LineStyle; label: string; dash?: string }> = [
  { value: "solid", label: "Solid line" },
  { value: "dashed", label: "Dashed line", dash: "4 3" },
  { value: "dotted", label: "Dotted line", dash: "0 3.5" },
];

function LineSample({ dash, bothEnds = false, oneEnd = false }: { dash?: string; bothEnds?: boolean; oneEnd?: boolean }) {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 12" className="tw:h-3 tw:w-6" fill="currentColor" stroke="currentColor">
      <line x1={bothEnds ? 6 : 2} y1="6" x2={bothEnds || oneEnd ? 18 : 22} y2="6" strokeWidth="2" strokeLinecap="round" strokeDasharray={dash} />
      {bothEnds || oneEnd ? <polygon points="23,6 17,2.5 17,9.5" strokeWidth="0" /> : null}
      {bothEnds ? <polygon points="1,6 7,2.5 7,9.5" strokeWidth="0" /> : null}
    </svg>
  );
}

/** A shape's line style; an arrow's heads; a box's or ellipse's fill and border. */
export function ShapeLookChoices({ style, onStyle, layout }: {
  style: ActiveStyle; onStyle: (patch: StylePatch) => void; layout: "row" | "sheet";
}) {
  const shape = style.shape;
  if (!shape) return null;
  const line = style.style.line ?? "solid";
  const frame = shape === "rectangle" || shape === "ellipse";
  const fill = style.style.fill ?? null;
  const border = style.style.border ?? true;
  const section = (title: string, children: ReactNode) => layout === "sheet" ? (
    <div className="tw:grid tw:gap-2">
      <p className="tw:m-0 tw:text-label tw:font-medium tw:text-muted-foreground">{title}</p>
      {children}
    </div>
  ) : children;
  return (
    <>
      {section("Line", (
        <Toolbar label="Line style" className="tw:flex-nowrap tw:gap-1">
          {LINE_STYLES.map((choice) => (
            <ToolbarAction key={choice.value} label={choice.label} pressed={line === choice.value}
              onClick={() => onStyle({ line: choice.value })}>
              <LineSample dash={choice.dash} />
            </ToolbarAction>
          ))}
        </Toolbar>
      ))}
      {shape === "arrow" ? section("Arrowheads", (
        <Toolbar label="Arrowheads" className="tw:flex-nowrap tw:gap-1">
          <ToolbarAction label="Head at one end" pressed={!style.style.bothEnds} onClick={() => onStyle({ bothEnds: false })}>
            <LineSample oneEnd />
          </ToolbarAction>
          <ToolbarAction label="Heads at both ends" pressed={style.style.bothEnds === true} onClick={() => onStyle({ bothEnds: true })}>
            <LineSample bothEnds />
          </ToolbarAction>
        </Toolbar>
      )) : null}
      {shape === "curve" ? section("Arrowheads", (
        <Toolbar label="Arrowheads" className="tw:flex-nowrap tw:gap-1">
          <ToolbarAction label="No arrowheads" pressed={(style.style.heads ?? "none") === "none"} onClick={() => onStyle({ heads: "none" })}>
            <LineSample />
          </ToolbarAction>
          <ToolbarAction label="Head at one end" pressed={style.style.heads === "end"} onClick={() => onStyle({ heads: "end" })}>
            <LineSample oneEnd />
          </ToolbarAction>
          <ToolbarAction label="Heads at both ends" pressed={style.style.heads === "both"} onClick={() => onStyle({ heads: "both" })}>
            <LineSample bothEnds />
          </ToolbarAction>
        </Toolbar>
      )) : null}
      {frame ? section("Fill", (
        <div className="tw:flex tw:flex-nowrap tw:items-center tw:gap-2">
          <Toolbar label="Fill" className="tw:flex-nowrap tw:gap-1">
            <ToolbarAction label="No fill" pressed={fill === null} onClick={() => onStyle({ fill: null, border: true })}>
              <span aria-hidden="true" className="tw:size-4 tw:rounded-sm tw:border-2 tw:border-solid tw:border-current" />
            </ToolbarAction>
            <ToolbarAction label="Filled" pressed={fill !== null}
              onClick={() => { if (fill === null) onStyle({ fill: lightFill(style.style.color) }); }}>
              <span aria-hidden="true" className="tw:size-4 tw:rounded-sm tw:border-2 tw:border-solid"
                style={{ backgroundColor: fill ?? lightFill(style.style.color), borderColor: border ? style.style.color : "transparent" }} />
            </ToolbarAction>
          </Toolbar>
          {fill !== null ? <PickedColor label="Fill color" value={fill} onPick={(color) => onStyle({ fill: color })} /> : null}
          <label className={cn("tw:flex tw:items-center tw:gap-2 tw:text-label", fill === null && "tw:text-muted-foreground")}>
            <Switch checked={border} disabled={fill === null} onCheckedChange={(checked) => onStyle({ border: checked })} />
            Border
          </label>
        </div>
      )) : null}
    </>
  );
}

/** A pale version of the outline color, so a new fill never hides what is under it completely. */
export function lightFill(color: string): string {
  const channel = (offset: number) => {
    const value = Number.parseInt(color.slice(offset, offset + 2), 16);
    return Math.round(value + (255 - value) * 0.8).toString(16).padStart(2, "0");
  };
  return /^#[0-9a-f]{6}$/.test(color) ? `#${channel(1)}${channel(3)}${channel(5)}` : "#f1f5f9";
}

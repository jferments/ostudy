import {
  clampToPage,
  handlesFor,
  isDeliberateShape,
  isShapeTool,
  isStickyNote,
  isStorable,
  MAX_STROKE_POINTS,
  moveWithinPage,
  resizeShape,
  round,
  shapeFromDrag,
  simplifyPairs,
  topmostAt,
  type Annotation,
  type AnnotationShape,
  type AnnotatorTool,
  type Handle,
  type HighlightShape,
  type InkShape,
  type Point,
  type ShapeTool,
  type Size,
  type ToolStyles,
} from "./annotationModel";
import { characterAt, characterNear, textBetween, type PageText } from "./textSelection";

export type PointerKind = "mouse" | "pen" | "touch";

export interface PointerSample {
  id: number;
  kind: PointerKind;
  clientX: number;
  clientY: number;
  /** 0 to 1, as the pointer reports it. */
  pressure: number;
  /** The button that went down: 0 primary, 1 middle, 5 a stylus's eraser end. */
  button: number;
  shiftKey: boolean;
  /** Milliseconds; only differences matter. */
  time: number;
}

/** What gestures need to know about the pages on screen. */
export interface GestureSurface {
  /** The page under a client position, if any. */
  pageAt(clientX: number, clientY: number): number | null;
  /** The page closest to a client position. */
  nearestPage(clientX: number, clientY: number): number | null;
  /** A client position in points on a page; it may lie outside the page. */
  toPage(pageNumber: number, clientX: number, clientY: number): Point;
  pageSize(pageNumber: number): Size | null;
}

export interface GestureContext {
  tool: AnnotatorTool;
  styles: ToolStyles;
  /** Fingers pan and zoom instead of drawing. */
  penOnly: boolean;
  /** CSS pixels per point. */
  scale: number;
  minScale: number;
  maxScale: number;
  selected: Annotation | null;
  annotationsOn(pageNumber: number): readonly Annotation[];
  textHeight(annotation: Annotation): number;
  /** The page's text, when it is known; the highlighter then snaps to it. */
  pageText?(pageNumber: number): PageText | null;
  newId(): string;
  now(): number;
}

/** The anchor that stays under the fingers while they zoom. */
export interface ZoomAnchor { pageNumber: number; point: Point; }

/** Something the screen should show or do after a pointer event. */
export type GestureEffect =
  | { kind: "draft"; draft: { pageNumber: number; shape: AnnotationShape; complete: boolean } | null }
  | { kind: "create"; annotation: Annotation }
  /** The selected annotation as it is being moved or reshaped. */
  | { kind: "preview"; annotation: Annotation | null }
  | { kind: "change"; before: Annotation; after: Annotation }
  /** Annotations the eraser has passed over; they are removed when it lifts. */
  | { kind: "erasing"; annotations: readonly Annotation[] }
  | { kind: "erase"; annotations: readonly Annotation[] }
  | { kind: "select"; id: string | null }
  | { kind: "edit-text"; annotation: Annotation }
  | { kind: "new-text"; pageNumber: number; point: Point; sticky: boolean }
  | { kind: "scroll-by"; dx: number; dy: number }
  /** Pixels per millisecond the view keeps moving after a flick. */
  | { kind: "fling"; velocityX: number; velocityY: number }
  | { kind: "zoom"; scale: number; anchor: ZoomAnchor; clientX: number; clientY: number }
  | { kind: "pen-seen" };

interface Client { x: number; y: number; }

type Gesture =
  | { kind: "ink"; pointerId: number; pageNumber: number; points: number[]; simulatePressure: boolean; last: Point }
  | { kind: "highlight"; pointerId: number; pageNumber: number; points: number[]; last: Point }
  /** A highlighter drag that began on the page's text, from one character to another. */
  | { kind: "text-highlight"; pointerId: number; pointer: PointerKind; start: Client; moved: boolean; pageNumber: number; text: PageText; from: number; to: number }
  | { kind: "shape"; pointerId: number; pageNumber: number; tool: ShapeTool; start: Point; shape: AnnotationShape | null }
  | { kind: "erase"; pointerId: number; last: Client; erased: Map<string, Annotation> }
  | { kind: "press"; pointerId: number; pointer: PointerKind; start: Client; pageNumber: number | null; hit: Annotation | null; purpose: "select" | "text" | "sticky" }
  | { kind: "move"; pointerId: number; pageNumber: number; original: Annotation; start: Point; current: Annotation }
  | { kind: "resize"; pointerId: number; pageNumber: number; original: Annotation; handle: Handle; current: Annotation }
  | { kind: "pan"; pointerId: number; last: Client; samples: Array<Client & { time: number }> }
  | { kind: "pinch"; ids: [number, number]; startDistance: number; startScale: number; anchor: ZoomAnchor | null }
  /** Pointers that do nothing until they lift, such as a resting palm. */
  | { kind: "idle" };

/** The button a stylus reports when its eraser end touches the screen. */
const PEN_ERASER_BUTTON = 5;
const TAP_SLOP = { mouse: 4, pen: 4, touch: 10 } as const;
const HIT_SLOP = { mouse: 6, pen: 6, touch: 14 } as const;
const ERASER_RADIUS = { mouse: 8, pen: 8, touch: 14 } as const;
/** Pen points closer than this many CSS pixels to the last one add nothing visible. */
const MIN_POINT_STEP = 0.75;
const FLING_WINDOW_MS = 80;
const MIN_FLING_SPEED = 0.25;

const clientDistance = (a: Client, b: Client) => Math.hypot(a.x - b.x, a.y - b.y);
const clientMidpoint = (a: Client, b: Client): Client => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });

function isDrawingTool(tool: AnnotatorTool): boolean {
  return tool !== "select";
}

/**
 * The annotator's pointer gestures as a pure state machine: pen, finger and
 * mouse input in, effects out. One finger or a pen draws with the drawing
 * tools; two fingers pan and zoom, and a second finger cancels an unfinished
 * stroke rather than drawing. With pen-only drawing, or while a pen is down,
 * fingers never draw. Only a lifted pointer commits anything.
 */
export class AnnotatorGestures {
  private readonly pointers = new Map<number, { kind: PointerKind; client: Client }>();
  private active: Gesture | null = null;

  /** True while a gesture would be disturbed by a re-render that moves the pages. */
  busy(): boolean {
    return this.active !== null && this.active.kind !== "idle";
  }

  down(sample: PointerSample, surface: GestureSurface, context: GestureContext): GestureEffect[] {
    const effects: GestureEffect[] = [];
    if (sample.kind === "pen") effects.push({ kind: "pen-seen" });
    if (this.pointers.has(sample.id)) return effects;
    const client = { x: sample.clientX, y: sample.clientY };
    this.pointers.set(sample.id, { kind: sample.kind, client });

    const active = this.active;
    if (active) {
      const owner = this.ownerKind(active);
      if (sample.kind === "pen" && owner !== "pen" && owner !== "mouse") {
        // A pen outranks the fingers (and palm) already on the screen.
        effects.push(...this.abandon());
        this.pointers.forEach((pointer, id) => { if (pointer.kind === "touch") this.pointers.delete(id); });
      } else if (sample.kind === "touch" && owner === "touch" && active.kind !== "pinch") {
        effects.push(...this.abandon());
        effects.push(...this.startPinch(surface, context));
        return effects;
      } else {
        return effects;
      }
    }

    if (this.pointers.size > 1 && sample.kind === "touch") {
      // A finger joining one the gesture already ignored, such as a palm.
      this.active = { kind: "idle" };
      return effects;
    }
    effects.push(...this.start(sample, client, surface, context));
    return effects;
  }

  move(sample: PointerSample, surface: GestureSurface, context: GestureContext): GestureEffect[] {
    const pointer = this.pointers.get(sample.id);
    if (!pointer) return [];
    const client = { x: sample.clientX, y: sample.clientY };
    pointer.client = client;
    const active = this.active;
    if (!active) return [];
    if (active.kind === "pinch") return this.pinch(active, context);
    if (active.kind === "idle" || active.pointerId !== sample.id) return [];

    switch (active.kind) {
      case "ink":
      case "highlight": {
        const size = surface.pageSize(active.pageNumber);
        if (!size) return [];
        const point = clampToPage(surface.toPage(active.pageNumber, client.x, client.y), size);
        const stride = active.kind === "ink" ? 3 : 2;
        if (active.points.length / stride >= MAX_STROKE_POINTS) return [];
        if (Math.hypot(point.x - active.last.x, point.y - active.last.y) * context.scale < MIN_POINT_STEP) return [];
        active.last = point;
        if (active.kind === "ink") active.points.push(point.x, point.y, inkPressure(sample, active.simulatePressure));
        else active.points.push(point.x, point.y);
        return [{ kind: "draft", draft: { pageNumber: active.pageNumber, shape: this.stroke(active, context), complete: false } }];
      }
      case "text-highlight": {
        if (!active.moved && clientDistance(client, active.start) < TAP_SLOP[active.pointer]) return [];
        active.moved = true;
        const to = characterNear(active.text, surface.toPage(active.pageNumber, client.x, client.y));
        if (to === null || to === active.to) return [];
        active.to = to;
        return [{ kind: "draft", draft: { pageNumber: active.pageNumber, shape: this.textHighlight(active, context), complete: false } }];
      }
      case "shape": {
        const size = surface.pageSize(active.pageNumber);
        if (!size) return [];
        const end = clampToPage(surface.toPage(active.pageNumber, client.x, client.y), size);
        const shape = shapeFromDrag(active.tool, active.start, end, sample.shiftKey, context.styles.shape, size);
        active.shape = isDeliberateShape(shape) ? shape : null;
        return [{ kind: "draft", draft: active.shape ? { pageNumber: active.pageNumber, shape: active.shape, complete: false } : null }];
      }
      case "erase":
        return this.erase(active, client, pointer.kind, surface, context);
      case "press": {
        if (clientDistance(client, active.start) < TAP_SLOP[active.pointer]) return [];
        if (active.purpose === "select" && active.hit && active.pageNumber !== null) {
          const effects: GestureEffect[] = [];
          if (context.selected?.id !== active.hit.id) effects.push({ kind: "select", id: active.hit.id });
          const start = surface.toPage(active.pageNumber, active.start.x, active.start.y);
          this.active = { kind: "move", pointerId: active.pointerId, pageNumber: active.pageNumber, original: active.hit, start, current: active.hit };
          return [...effects, ...this.move(sample, surface, context)];
        }
        if (active.pointer === "touch") {
          this.active = { kind: "pan", pointerId: active.pointerId, last: active.start, samples: [] };
          return this.move(sample, surface, context);
        }
        this.active = { kind: "idle" };
        return [];
      }
      case "move": {
        const size = surface.pageSize(active.pageNumber);
        if (!size) return [];
        const point = surface.toPage(active.pageNumber, client.x, client.y);
        const shape = moveWithinPage(
          active.original.shape, point.x - active.start.x, point.y - active.start.y, size, context.textHeight(active.original),
        );
        active.current = { ...active.original, shape };
        return [{ kind: "preview", annotation: active.current }];
      }
      case "resize": {
        const size = surface.pageSize(active.pageNumber);
        if (!size) return [];
        const point = clampToPage(surface.toPage(active.pageNumber, client.x, client.y), size);
        active.current = { ...active.original, shape: resizeShape(active.original.shape, active.handle, point, sample.shiftKey, size) };
        return [{ kind: "preview", annotation: active.current }];
      }
      case "pan": {
        const effect: GestureEffect = { kind: "scroll-by", dx: active.last.x - client.x, dy: active.last.y - client.y };
        active.last = client;
        active.samples.push({ ...client, time: sample.time });
        while (active.samples.length > 2 && sample.time - (active.samples[0]?.time ?? sample.time) > FLING_WINDOW_MS) {
          active.samples.shift();
        }
        return [effect];
      }
    }
  }

  up(sample: PointerSample, surface: GestureSurface, context: GestureContext): GestureEffect[] {
    if (!this.pointers.has(sample.id)) return [];
    this.pointers.delete(sample.id);
    const active = this.active;
    if (!active) return [];
    if (active.kind === "pinch") {
      // The finger left behind does not draw or pan.
      if (active.ids.includes(sample.id)) this.active = this.pointers.size > 0 ? { kind: "idle" } : null;
      return [];
    }
    if (active.kind === "idle" || active.pointerId !== sample.id) {
      if (this.pointers.size === 0) this.active = null;
      return [];
    }
    this.active = this.pointers.size > 0 ? { kind: "idle" } : null;

    switch (active.kind) {
      case "ink":
      case "highlight": {
        const shape = this.stroke(active, context, true);
        const keep = shape.kind === "ink" || shape.points.length >= 4;
        return [
          { kind: "draft", draft: null },
          ...(keep ? [this.create(active.pageNumber, shape, context)] : []),
        ];
      }
      case "text-highlight": {
        // A tap on the text marks nothing; only a drag across it does.
        const shape = this.textHighlight(active, context);
        return [
          { kind: "draft", draft: null },
          ...(active.moved && isStorable(shape) ? [this.create(active.pageNumber, shape, context)] : []),
        ];
      }
      case "shape":
        return [
          { kind: "draft", draft: null },
          ...(active.shape ? [this.create(active.pageNumber, active.shape, context)] : []),
        ];
      case "erase":
        return [
          { kind: "erasing", annotations: [] },
          ...(active.erased.size > 0 ? [{ kind: "erase", annotations: [...active.erased.values()] } as const] : []),
        ];
      case "press":
        return this.tap(active, surface, context);
      case "move":
      case "resize":
        return [
          { kind: "preview", annotation: null },
          ...(sameShape(active.current.shape, active.original.shape)
            ? []
            : [{ kind: "change", before: active.original, after: active.current } as const]),
        ];
      case "pan": {
        const first = active.samples[0];
        const last = active.samples[active.samples.length - 1];
        if (!first || !last || last.time <= first.time || sample.time - last.time > FLING_WINDOW_MS) return [];
        const velocityX = (first.x - last.x) / (last.time - first.time);
        const velocityY = (first.y - last.y) / (last.time - first.time);
        return Math.hypot(velocityX, velocityY) >= MIN_FLING_SPEED ? [{ kind: "fling", velocityX, velocityY }] : [];
      }
    }
  }

  /** The browser took the pointer away (a system gesture, a lost capture): nothing commits. */
  cancel(pointerId: number): GestureEffect[] {
    if (!this.pointers.has(pointerId)) return [];
    this.pointers.delete(pointerId);
    const active = this.active;
    if (!active) return [];
    if (active.kind === "idle" || !this.involves(active, pointerId)) {
      if (this.pointers.size === 0) this.active = null;
      return [];
    }
    const effects = this.abandon();
    this.active = this.pointers.size > 0 ? { kind: "idle" } : null;
    return effects;
  }

  /** Drops every gesture, as when the tool changes or the annotator closes. */
  reset(): GestureEffect[] {
    const effects = this.abandon();
    this.pointers.clear();
    this.active = null;
    return effects;
  }

  private start(sample: PointerSample, client: Client, surface: GestureSurface, context: GestureContext): GestureEffect[] {
    const base = { pointerId: sample.id };
    if (sample.kind === "mouse" && sample.button === 1) {
      this.active = { kind: "pan", ...base, last: client, samples: [] };
      return [];
    }
    if (sample.kind === "mouse" && sample.button !== 0) {
      this.active = { kind: "idle" };
      return [];
    }
    // A stylus's eraser end erases, whichever tool is chosen.
    const tool: AnnotatorTool = sample.kind === "pen" && sample.button === PEN_ERASER_BUTTON ? "eraser" : context.tool;
    const placesNotes = tool === "text" || tool === "sticky";
    // With pen-only drawing a finger still places and opens notes.
    const fingerPans = sample.kind === "touch" && context.penOnly && isDrawingTool(tool) && !placesNotes;
    const pageNumber = surface.pageAt(client.x, client.y);
    if (fingerPans || (pageNumber === null && isDrawingTool(tool) && !placesNotes)) {
      this.active = sample.kind === "touch"
        ? { kind: "pan", ...base, last: client, samples: [{ ...client, time: sample.time }] }
        : { kind: "idle" };
      return [];
    }
    const size = pageNumber === null ? null : surface.pageSize(pageNumber);
    const point = pageNumber === null ? null : surface.toPage(pageNumber, client.x, client.y);

    if (tool === "select" || placesNotes) {
      let hit: Annotation | null = null;
      if (pageNumber !== null && point && size) {
        const slop = HIT_SLOP[sample.kind] / context.scale;
        const selected = tool === "select" && context.selected?.pageNumber === pageNumber ? context.selected : null;
        const handle = selected ? handleAt(selected, point, slop, context) : null;
        if (selected && handle) {
          this.active = { kind: "resize", ...base, pageNumber, original: selected, handle, current: selected };
          return [];
        }
        hit = topmostAt(
          context.annotationsOn(pageNumber), point, slop, context.textHeight,
          tool === "select" ? undefined
            : (annotation) => annotation.shape.kind === "text" && (tool === "text" || isStickyNote(annotation.shape)),
        );
      }
      this.active = { kind: "press", ...base, pointer: sample.kind, start: client, pageNumber, hit, purpose: tool };
      return [];
    }

    if (pageNumber === null || !point || !size) {
      this.active = { kind: "idle" };
      return [];
    }
    const start = clampToPage(point, size);
    if (tool === "eraser") {
      const gesture: Gesture = { kind: "erase", ...base, last: client, erased: new Map() };
      this.active = gesture;
      return this.erase(gesture, client, sample.kind, surface, context);
    }
    if (isShapeTool(tool)) {
      this.active = { kind: "shape", ...base, pageNumber, tool, start, shape: null };
      return [];
    }
    if (tool === "pen") {
      const simulatePressure = sample.kind !== "pen";
      const gesture: Gesture = {
        kind: "ink", ...base, pageNumber, last: start, simulatePressure,
        points: [start.x, start.y, inkPressure(sample, simulatePressure)],
      };
      this.active = gesture;
      return [{ kind: "draft", draft: { pageNumber, shape: this.stroke(gesture, context), complete: false } }];
    }
    const text = context.pageText?.(pageNumber) ?? null;
    const first = text ? characterAt(text, point, HIT_SLOP[sample.kind] / context.scale) : null;
    if (text && first !== null) {
      // Nothing shows until the drag moves, so a tap leaves no mark behind.
      this.active = { kind: "text-highlight", ...base, pointer: sample.kind, start: client, moved: false, pageNumber, text, from: first, to: first };
      return [];
    }
    const gesture: Gesture = { kind: "highlight", ...base, pageNumber, last: start, points: [start.x, start.y] };
    this.active = gesture;
    return [{ kind: "draft", draft: { pageNumber, shape: this.stroke(gesture, context), complete: false } }];
  }

  private startPinch(surface: GestureSurface, context: GestureContext): GestureEffect[] {
    const touches = [...this.pointers.entries()].filter(([, pointer]) => pointer.kind === "touch").slice(-2);
    const [a, b] = touches;
    if (!a || !b) {
      this.active = { kind: "idle" };
      return [];
    }
    const middle = clientMidpoint(a[1].client, b[1].client);
    const pageNumber = surface.nearestPage(middle.x, middle.y);
    this.active = {
      kind: "pinch",
      ids: [a[0], b[0]],
      startDistance: Math.max(1, clientDistance(a[1].client, b[1].client)),
      startScale: context.scale,
      anchor: pageNumber === null ? null : { pageNumber, point: surface.toPage(pageNumber, middle.x, middle.y) },
    };
    return [];
  }

  private pinch(active: Extract<Gesture, { kind: "pinch" }>, context: GestureContext): GestureEffect[] {
    const a = this.pointers.get(active.ids[0]);
    const b = this.pointers.get(active.ids[1]);
    if (!a || !b || !active.anchor) return [];
    const middle = clientMidpoint(a.client, b.client);
    const scale = Math.min(context.maxScale, Math.max(context.minScale,
      active.startScale * clientDistance(a.client, b.client) / active.startDistance));
    return [{ kind: "zoom", scale, anchor: active.anchor, clientX: middle.x, clientY: middle.y }];
  }

  private erase(
    active: Extract<Gesture, { kind: "erase" }>,
    client: Client,
    pointer: PointerKind,
    surface: GestureSurface,
    context: GestureContext,
  ): GestureEffect[] {
    const radius = ERASER_RADIUS[pointer];
    const steps = Math.max(1, Math.ceil(clientDistance(active.last, client) / (radius / 2)));
    const before = active.erased.size;
    for (let step = 1; step <= steps; step += 1) {
      const x = active.last.x + (client.x - active.last.x) * step / steps;
      const y = active.last.y + (client.y - active.last.y) * step / steps;
      const pageNumber = surface.pageAt(x, y);
      if (pageNumber === null) continue;
      const point = surface.toPage(pageNumber, x, y);
      for (;;) {
        const hit = topmostAt(
          context.annotationsOn(pageNumber), point, radius / context.scale, context.textHeight,
          (annotation) => !active.erased.has(annotation.id),
        );
        if (!hit) break;
        active.erased.set(hit.id, hit);
      }
    }
    active.last = client;
    return active.erased.size === before ? [] : [{ kind: "erasing", annotations: [...active.erased.values()] }];
  }

  private tap(active: Extract<Gesture, { kind: "press" }>, surface: GestureSurface, context: GestureContext): GestureEffect[] {
    if (active.purpose !== "select") {
      if (active.hit) return [{ kind: "edit-text", annotation: active.hit }];
      if (active.pageNumber === null) return [];
      const size = surface.pageSize(active.pageNumber);
      if (!size) return [];
      const point = clampToPage(surface.toPage(active.pageNumber, active.start.x, active.start.y), size);
      return [{ kind: "new-text", pageNumber: active.pageNumber, point, sticky: active.purpose === "sticky" }];
    }
    if (!active.hit) return context.selected ? [{ kind: "select", id: null }] : [];
    // A tap on the selected text note opens it, so a double tap edits.
    if (active.hit.shape.kind === "text" && context.selected?.id === active.hit.id) {
      return [{ kind: "edit-text", annotation: active.hit }];
    }
    return context.selected?.id === active.hit.id ? [] : [{ kind: "select", id: active.hit.id }];
  }

  private stroke(
    active: Extract<Gesture, { kind: "ink" | "highlight" }>,
    context: GestureContext,
    finished = false,
  ): InkShape | HighlightShape {
    if (active.kind === "ink") {
      return {
        kind: "ink",
        points: [...active.points],
        color: context.styles.pen.color,
        strokeWidth: context.styles.pen.size,
        simulatePressure: active.simulatePressure,
      };
    }
    return {
      kind: "highlight",
      points: finished ? simplifyPairs(active.points, 0.4) : [...active.points],
      color: context.styles.highlighter.color,
      strokeWidth: context.styles.highlighter.size,
    };
  }

  private textHighlight(active: Extract<Gesture, { kind: "text-highlight" }>, context: GestureContext): HighlightShape {
    const { rects, text } = textBetween(active.text, active.from, active.to);
    return {
      kind: "highlight",
      points: [],
      rects,
      text,
      color: context.styles.highlighter.color,
      strokeWidth: context.styles.highlighter.size,
    };
  }

  private create(pageNumber: number, shape: AnnotationShape, context: GestureContext): GestureEffect {
    return { kind: "create", annotation: { id: context.newId(), pageNumber, shape, createdAt: context.now() } };
  }

  /** Effects that clear whatever an unfinished gesture was showing. */
  private abandon(): GestureEffect[] {
    const active = this.active;
    this.active = null;
    switch (active?.kind) {
      case "ink":
      case "highlight":
      case "text-highlight":
      case "shape":
        return [{ kind: "draft", draft: null }];
      case "erase":
        return [{ kind: "erasing", annotations: [] }];
      case "move":
      case "resize":
        return [{ kind: "preview", annotation: null }];
      default:
        return [];
    }
  }

  private ownerKind(active: Gesture): PointerKind | null {
    if (active.kind === "idle") return null;
    if (active.kind === "pinch") return "touch";
    return this.pointers.get(active.pointerId)?.kind ?? null;
  }

  private involves(active: Exclude<Gesture, { kind: "idle" }>, pointerId: number): boolean {
    return active.kind === "pinch" ? active.ids.includes(pointerId) : active.pointerId === pointerId;
  }
}

function handleAt(selected: Annotation, point: Point, slop: number, context: GestureContext): Handle | null {
  const height = context.textHeight(selected);
  for (const { handle, point: at } of handlesFor(selected.shape, height)) {
    if (Math.hypot(point.x - at.x, point.y - at.y) <= slop * 1.5) return handle;
  }
  return null;
}

/**
 * A pen's pressure, or a steady 0.5 for input without it. Some pens report 0
 * on their first sample; that would start every stroke as a hairline.
 */
function inkPressure(sample: PointerSample, simulated: boolean): number {
  if (simulated || !(sample.pressure > 0)) return 0.5;
  return round(Math.min(1, sample.pressure));
}

function sameShape(a: AnnotationShape, b: AnnotationShape): boolean {
  return a === b || JSON.stringify(a) === JSON.stringify(b);
}


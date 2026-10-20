import { getDocumentPageText, type AnnotatedPage, type DocumentPageText } from "../../../ipc/documentAnnotations";
import { MAX_TEXT_CHARS } from "./annotationModel";

/**
 * A page's characters in reading order, with boxes in the annotator's page
 * points: `boxes` holds `x, y, width, height` per character, and a zero
 * width marks a space or line break the PDF only implies.
 */
export interface PageText {
  characters: readonly string[];
  boxes: Float64Array;
}

/** The page text in the annotator's page points, which come from the page image's size. */
export function toPageText(page: Pick<AnnotatedPage, "width" | "height">, text: DocumentPageText): PageText {
  const characters = Array.from(text.characters);
  const count = Math.min(characters.length, Math.floor(text.boxes.length / 4));
  const sx = text.width > 0 ? page.width / text.width : 1;
  const sy = text.height > 0 ? page.height / text.height : 1;
  const boxes = new Float64Array(count * 4);
  for (let index = 0; index < count; index += 1) {
    boxes[index * 4] = (text.boxes[index * 4] ?? 0) * sx;
    boxes[index * 4 + 1] = (text.boxes[index * 4 + 1] ?? 0) * sy;
    boxes[index * 4 + 2] = (text.boxes[index * 4 + 2] ?? 0) * sx;
    boxes[index * 4 + 3] = (text.boxes[index * 4 + 3] ?? 0) * sy;
  }
  return { characters: characters.slice(0, count), boxes };
}

function distanceToCharacter(text: PageText, index: number, x: number, y: number): number {
  const left = text.boxes[index * 4] ?? 0;
  const top = text.boxes[index * 4 + 1] ?? 0;
  const right = left + (text.boxes[index * 4 + 2] ?? 0);
  const bottom = top + (text.boxes[index * 4 + 3] ?? 0);
  return Math.hypot(Math.max(left - x, 0, x - right), Math.max(top - y, 0, y - bottom));
}

function drawn(text: PageText, index: number): boolean {
  return (text.boxes[index * 4 + 2] ?? 0) > 0 && (text.characters[index] ?? "").trim() !== "";
}

/** The drawn character nearest a point within `slop` points of it, or null when the point is off the text. */
export function characterAt(text: PageText, point: { x: number; y: number }, slop: number): number | null {
  let best: number | null = null;
  let bestDistance = slop;
  for (let index = 0; index < text.characters.length; index += 1) {
    if (!drawn(text, index)) continue;
    const distance = distanceToCharacter(text, index, point.x, point.y);
    if (distance <= bestDistance) {
      best = index;
      bestDistance = distance;
      if (distance === 0) break;
    }
  }
  return best;
}

/**
 * The drawn character a drag has reached: the nearest one on the line under
 * the point, or else the nearest anywhere, so a drag past the end of a line
 * or between lines still selects in reading order.
 */
export function characterNear(text: PageText, point: { x: number; y: number }): number | null {
  let best: number | null = null;
  let bestDistance = Infinity;
  let onLine: number | null = null;
  let onLineDistance = Infinity;
  for (let index = 0; index < text.characters.length; index += 1) {
    if (!drawn(text, index)) continue;
    const top = text.boxes[index * 4 + 1] ?? 0;
    const bottom = top + (text.boxes[index * 4 + 3] ?? 0);
    const distance = distanceToCharacter(text, index, point.x, point.y);
    if (point.y >= top && point.y <= bottom && distance < onLineDistance) {
      onLine = index;
      onLineDistance = distance;
    }
    if (distance < bestDistance) {
      best = index;
      bestDistance = distance;
    }
  }
  return onLine ?? best;
}

const round = (value: number) => Math.round(value * 100) / 100;

/**
 * The boxes and words from one character to another, either way round: one
 * box per run of characters on a line, and the text with its line breaks as
 * spaces.
 */
export function textBetween(text: PageText, a: number, b: number): { rects: number[]; text: string } {
  const from = Math.max(0, Math.min(a, b));
  const to = Math.min(text.characters.length - 1, Math.max(a, b));
  const rects: number[] = [];
  let run: { left: number; top: number; right: number; bottom: number } | null = null;
  const flush = () => {
    if (run) rects.push(round(run.left), round(run.top), round(run.right - run.left), round(run.bottom - run.top));
    run = null;
  };
  for (let index = from; index <= to; index += 1) {
    const width = text.boxes[index * 4 + 2] ?? 0;
    if (width <= 0) continue;
    const left = text.boxes[index * 4] ?? 0;
    const top = text.boxes[index * 4 + 1] ?? 0;
    const right = left + width;
    const bottom = top + (text.boxes[index * 4 + 3] ?? 0);
    const middle = (top + bottom) / 2;
    const current: { left: number; top: number; right: number; bottom: number } | null = run;
    // The same line: vertically overlapping and not jumping back to the left.
    if (current && middle >= current.top && middle <= current.bottom && left >= current.left - 1) {
      current.left = Math.min(current.left, left);
      current.top = Math.min(current.top, top);
      current.right = Math.max(current.right, right);
      current.bottom = Math.max(current.bottom, bottom);
    } else {
      flush();
      run = { left, top, right, bottom };
    }
  }
  flush();
  const words = text.characters.slice(from, to + 1).join("")
    .replace(/\s+/gu, " ")
    .replace(/\p{Cc}/gu, "")
    .trim();
  return { rects, text: Array.from(words).slice(0, MAX_TEXT_CHARS).join("") };
}

/** Pages whose text stays loaded; a dense page's boxes take a megabyte or two. */
const PAGE_TEXT_LIMIT = 16;

export interface PageTextCache {
  /** The page's text once loaded; null when it has none or could not be read. */
  get(pageNumber: number): PageText | null;
  /** Loads the page's text if it is not loaded or loading already. */
  request(page: AnnotatedPage): void;
  dispose(): void;
}

/**
 * The text of the pages near the view, read from the PDF as the highlighter
 * needs it. A page that fails (a scan, a platform without text positions) is
 * remembered as having none and not asked for again.
 */
export function createPageTextCache(
  documentId: string,
  load: (documentId: string, pageNumber: number) => Promise<DocumentPageText> = getDocumentPageText,
): PageTextCache {
  const loaded = new Map<number, PageText | null>();
  const loading = new Set<number>();
  let disposed = false;
  return {
    get(pageNumber) {
      const text = loaded.get(pageNumber);
      if (text === undefined) return null;
      loaded.delete(pageNumber);
      loaded.set(pageNumber, text);
      return text;
    },
    request(page) {
      if (disposed || loaded.has(page.pageNumber) || loading.has(page.pageNumber)) return;
      loading.add(page.pageNumber);
      void Promise.resolve()
        .then(() => load(documentId, page.pageNumber))
        .then((text) => toPageText(page, text), () => null)
        .then((text) => {
          loading.delete(page.pageNumber);
          if (disposed) return;
          loaded.set(page.pageNumber, text && text.characters.length > 0 ? text : null);
          for (const pageNumber of loaded.keys()) {
            if (loaded.size <= PAGE_TEXT_LIMIT) break;
            loaded.delete(pageNumber);
          }
        });
    },
    dispose() {
      disposed = true;
      loaded.clear();
    },
  };
}

import { renderFormula } from "./render";
import { MAX_FORMULA_SOURCE_BYTES } from "./source";

const FORMULAS_PER_FRAME = 4;
const BYTES_PER_FRAME = 16_384;
interface Work { element: HTMLElement; source: string; block: boolean; bytes: number; }
const waiting = new Map<HTMLElement, Work>();
const ready = new Set<HTMLElement>();
let observer: IntersectionObserver | null = null;
let frame: number | null = null;

/** Bound active rendering work rather than rejecting a whole mathematical reply.
 * Offscreen equations remain readable source until approached. Cancellation drops
 * DOM/source references; this queue never stores completed learner content.
 */
export function scheduleFormula(element: HTMLElement, source: string, block: boolean): () => void {
  const work = { element, source, block, bytes: new TextEncoder().encode(source).byteLength };
  waiting.set(element, work);
  element.textContent = block ? `\\[${source}\\]` : `\\(${source}\\)`;
  element.setAttribute("aria-busy", "true");
  if (typeof IntersectionObserver === "undefined") {
    ready.add(element);
    scheduleFrame();
  } else {
    observer ??= new IntersectionObserver((entries) => {
      for (const entry of entries) {
        const node = entry.target as HTMLElement;
        if (entry.isIntersecting && waiting.has(node)) ready.add(node);
        else ready.delete(node);
      }
      scheduleFrame();
    }, { rootMargin: "300px" });
    observer.observe(element);
  }
  return () => {
    if (waiting.get(element) !== work) return;
    waiting.delete(element);
    ready.delete(element);
    observer?.unobserve(element);
    element.removeAttribute("aria-busy");
    releaseIdleResources();
  };
}

function scheduleFrame(): void {
  if (frame === null && ready.size > 0) frame = window.requestAnimationFrame(flush);
}

function flush(): void {
  frame = null;
  let count = 0;
  let bytes = 0;
  for (const element of ready) {
    const work = waiting.get(element);
    if (!work) { ready.delete(element); continue; }
    // The per-equation renderer rejects oversized source without evaluating it.
    const cost = Math.min(work.bytes, MAX_FORMULA_SOURCE_BYTES);
    if (count > 0 && (count >= FORMULAS_PER_FRAME || bytes + cost > BYTES_PER_FRAME)) break;
    ready.delete(element);
    waiting.delete(element);
    observer?.unobserve(element);
    if (element.isConnected) renderFormula(element, work.source, work.block);
    element.removeAttribute("aria-busy");
    count += 1;
    bytes += cost;
  }
  releaseIdleResources();
  scheduleFrame();
}

function releaseIdleResources(): void {
  if (waiting.size === 0) { observer?.disconnect(); observer = null; }
  if (ready.size === 0 && frame !== null) {
    window.cancelAnimationFrame(frame);
    frame = null;
  }
}

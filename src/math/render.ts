import katex from "katex";
import { formulaRenderingError, MAX_FORMULA_SOURCE_BYTES } from "./source";

/** Own the trust/resource limits once for every read-only mathematical surface. */
export function renderFormula(element: HTMLElement, source: string, block: boolean): string | null {
  let error = formulaRenderingError(source);
  element.replaceChildren();
  element.removeAttribute("data-formula-error");
  element.removeAttribute("aria-label");
  element.removeAttribute("title");
  if (!error) {
    try {
      katex.render(source, element, {
        displayMode: block,
        output: "htmlAndMathml",
        throwOnError: true,
        trust: false,
        strict: "error",
        maxExpand: 1000,
        maxSize: 10,
        // KaTeX can mutate macros. Never share them across cards or learners.
        macros: {},
      });
      return null;
    } catch {
      // Do not expose/log parser internals or user source through diagnostics.
      error = "This formula cannot be displayed. Its source is preserved.";
    }
  }
  const visible = source.length > MAX_FORMULA_SOURCE_BYTES
    ? `${source.slice(0, MAX_FORMULA_SOURCE_BYTES)}…` : source;
  element.textContent = block ? `\\[${visible}\\]` : `\\(${visible}\\)`;
  element.dataset.formulaError = "true";
  element.setAttribute("aria-label", `Formula source. ${error}`);
  element.title = error;
  return error;
}

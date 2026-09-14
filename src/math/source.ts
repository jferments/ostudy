/** Keep byte admission aligned with src-tauri/src/cards/math.rs. */
export const MAX_FORMULA_SOURCE_BYTES = 16 * 1024;

/** This admits source data, not executable TeX or a claim of typesetting validity. */
export function formulaSourceError(source: unknown): string | null {
  if (typeof source !== "string" || source.trim().length === 0) {
    return "Enter a formula before saving.";
  }
  // UTF-8 uses at least one byte per UTF-16 code unit. Bound work before encoding.
  if (source.length > MAX_FORMULA_SOURCE_BYTES) {
    return "A formula must not exceed 16,384 UTF-8 bytes.";
  }
  // TextEncoder replaces lone surrogates. Reject them rather than silently
  // persisting different source when JavaScript strings cross the UTF-8 IPC port.
  for (const character of source) {
    const code = character.codePointAt(0)!;
    if (code >= 0xd800 && code <= 0xdfff) return "A formula contains invalid Unicode.";
    if ((code < 0x20 && ![0x09, 0x0a, 0x0d].includes(code)) || (code >= 0x7f && code <= 0x9f)) {
      return "A formula contains invalid control characters.";
    }
  }
  if (new TextEncoder().encode(source).byteLength > MAX_FORMULA_SOURCE_BYTES) {
    return "A formula must not exceed 16,384 UTF-8 bytes.";
  }
  return null;
}

/** Renderer/editor admission is deliberately narrower than storage admission. */
export function formulaRenderingError(source: string): string | null {
  const admission = formulaSourceError(source);
  if (admission) return admission;
  // These are presentation engines, not an HTML, URL, macro-programming, or
  // system-TeX execution surface. Storage still preserves unsupported source.
  const forbidden = /\\(?:href|url|includegraphics|html[A-Za-z]*|css[A-Za-z]*|style|class|def|gdef|edef|xdef|let|futurelet|newcommand|renewcommand|providecommand|csname|catcode|input|include|write|openout|read|usepackage|require)\b/u;
  if (forbidden.test(source) || /<\/?[A-Za-z][^>]*>/u.test(source)) {
    return "This formula uses unsupported markup, links, or commands. Its source is preserved.";
  }
  let depth = 0;
  for (let index = 0; index < source.length; index += 1) {
    if (source[index] === "\\") { index += 1; continue; }
    if (source[index] === "{" && ++depth > 64) return "This formula is nested too deeply to display safely.";
    if (source[index] === "}") depth = Math.max(0, depth - 1);
  }
  return null;
}

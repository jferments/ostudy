import { Lexer, Marked, Tokenizer, type Token } from "marked";
import { inlineDollarEnd } from "./dollar";

export const MAX_STUDY_MARKDOWN_CHARS = 24_000;
export const MAX_MARKDOWN_FORMULAS = 64;
const MAX_TOTAL_FORMULA_BYTES = 32_768;
const FORMULA_TOKEN = "ostudyFormula";

export interface MarkdownFormula {
  type: typeof FORMULA_TOKEN;
  raw: string;
  source: string;
  block: boolean;
  complete: boolean;
}

export function isMarkdownFormula(token: Token): token is MarkdownFormula {
  return token.type === FORMULA_TOKEN;
}

/** Source offsets are presentation-only. Never rewrite saved text, evidence or hashes.
 * Use an isolated lexer: repeated renders must not register global extensions or
 * share work budgets. No Marked-generated HTML enters the application DOM.
 */
export function lexStudyMarkdown(text: string, inline = false): Token[] | null {
  if (text.length > MAX_STUDY_MARKDOWN_CHARS) return null;
  let formulas = 0;
  let formulaBytes = 0;
  function admit(token: MarkdownFormula): MarkdownFormula {
    if (token.complete) {
      formulas += 1;
      formulaBytes += new TextEncoder().encode(token.source).byteLength;
      if (formulas > MAX_MARKDOWN_FORMULAS || formulaBytes > MAX_TOTAL_FORMULA_BYTES) {
        throw new Error("Mathematical presentation budget exceeded.");
      }
    }
    return token;
  }
  const marked = new Marked({
    gfm: true, breaks: true,
    extensions: [
      {
        name: FORMULA_TOKEN, level: "block",
        start(src) { return /(?:^|\n) {0,3}(?:\\\[|\$\$)/u.exec(src)?.index; },
        tokenizer(src) {
          // Four-space indentation belongs to Markdown code, not mathematics.
          const prefix = /^ {0,3}(?=\\\[|\$\$)/u.exec(src)?.[0];
          if (prefix === undefined) return;
          const token = formulaAtStart(src.slice(prefix.length));
          if (!token?.complete || !/^(?:[ \t]*(?:\n|$))/u.test(src.slice(prefix.length + token.raw.length))) return;
          return admit({ ...token, raw: prefix + token.raw });
        },
      },
      {
        name: FORMULA_TOKEN, level: "inline",
        start(src) { return nextOpening(src); },
        tokenizer(src) {
          if (this.lexer.state.inRawBlock) return;
          const token = formulaAtStart(src);
          return token ? admit(token) : undefined;
        },
      },
    ],
    tokenizer: {
      // A multiplication star/subscript inside TeX must not close surrounding
      // Markdown emphasis. Keep Marked's length-aligned code/link masking too.
      emStrong(src, maskedSrc, prevChar) {
        if (!/^[*_]/u.test(src)) return false;
        return Tokenizer.prototype.emStrong.call(this, src, maskFormulas(src, maskedSrc), prevChar);
      },
      del(src, maskedSrc, prevChar) {
        if (!src.startsWith("~")) return false;
        return Tokenizer.prototype.del.call(this, src, maskFormulas(src, maskedSrc), prevChar);
      },
    },
  });
  try {
    const lexer = new Lexer(marked.defaults);
    return inline ? lexer.inlineTokens(text) : lexer.lex(text);
  } catch {
    // Malformed/oversized presentation remains full literal text, never a
    // truncated answer, a source-bearing diagnostic or a partially saved edit.
    return null;
  }
}

function escaped(text: string, position: number): boolean {
  let slashes = 0;
  while (position > 0 && text[--position] === "\\") slashes += 1;
  return slashes % 2 === 1;
}

function nextOpening(text: string, from = 0): number | undefined {
  const openings = /\\[([]|\$\$|\$/gu;
  openings.lastIndex = from;
  for (let match = openings.exec(text); match; match = openings.exec(text)) {
    if (escaped(text, match.index)) continue;
    if (match[0] === "$" && inlineDollarEnd(text.slice(match.index)) === undefined) continue;
    return match.index;
  }
  return undefined;
}

function formulaAtStart(text: string): MarkdownFormula | undefined {
  if (text[0] === "$" && text[1] !== "$") {
    const end = inlineDollarEnd(text);
    if (end === undefined) return;
    return { type: FORMULA_TOKEN, raw: text.slice(0, end + 1), source: text.slice(1, end), block: false, complete: true };
  }
  const open = text.slice(0, 2);
  const close = open === "\\(" ? "\\)" : open === "\\[" ? "\\]" : open === "$$" ? "$$" : null;
  if (!close) return;
  let end = text.indexOf(close, 2);
  while (end >= 0 && escaped(text, end)) end = text.indexOf(close, end + 2);
  if (end < 0) {
    // Preserve unfinished streamed formulas, including their backslashes. Do
    // not let Markdown reinterpret half a formula as links or emphasis.
    return { type: FORMULA_TOKEN, raw: text, source: "", block: open !== "\\(", complete: false };
  }
  return { type: FORMULA_TOKEN, raw: text.slice(0, end + 2), source: text.slice(2, end), block: open !== "\\(", complete: true };
}

function maskFormulas(src: string, maskedSrc: string): string {
  const offset = maskedSrc.length - src.length;
  if (offset < 0) return maskedSrc;
  let masked = maskedSrc;
  let position = nextOpening(src);
  while (position !== undefined) {
    const token = formulaAtStart(src.slice(position));
    if (!token?.complete) break;
    const start = offset + position;
    // Marked has already hidden code/links and replaced escaped punctuation
    // with '+'. Do not unmask them or treat a delimiter inside code as an opener.
    const visible = maskedSrc.slice(start, start + 2);
    if (visible === "++" || visible === "$$" || (src[position] === "$" && visible[0] === "$")) {
      masked = masked.slice(0, start) + "a".repeat(token.raw.length) + masked.slice(start + token.raw.length);
    }
    position = nextOpening(src, position + token.raw.length);
  }
  return masked;
}

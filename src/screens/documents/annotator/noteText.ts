import { inlineDollarEnd } from "../../../math/dollar";

/**
 * A text note's lines as they are drawn. The note's text stays what was typed;
 * a line that starts like a list item is drawn as one:
 *
 * - `- ` or `* ` is a bullet,
 * - `1. ` or `1) ` is a numbered item, with the number as typed,
 * - `[ ] ` or `[x] ` (also after `- `) is a checkbox, ticked with an x.
 *
 * Leading spaces indent a line, two to a level. Within a line, `$…$` is a
 * formula and `$$…$$` a formula on a line of its own.
 */
export type NoteLine =
  | { kind: "plain"; indent: number; body: string }
  | { kind: "bullet"; indent: number; body: string }
  | { kind: "number"; indent: number; number: string; body: string }
  | { kind: "check"; indent: number; checked: boolean; body: string };

export type NoteSpan = { text: string } | { math: string; block: boolean };

const CHECK = /^( *)(?:[-*] )?\[([ xX])\] (.*)$/u;
const BULLET = /^( *)[-*•] (.*)$/u;
const NUMBER = /^( *)(\d{1,3}[.)]) (.*)$/u;

export function noteLines(text: string): NoteLine[] {
  return text.split("\n").map((line): NoteLine => {
    const check = CHECK.exec(line);
    if (check) return { kind: "check", indent: level(check[1]!), checked: check[2] !== " ", body: check[3]! };
    const bullet = BULLET.exec(line);
    if (bullet) return { kind: "bullet", indent: level(bullet[1]!), body: bullet[2]! };
    const number = NUMBER.exec(line);
    if (number) return { kind: "number", indent: level(number[1]!), number: number[2]!, body: number[3]! };
    return { kind: "plain", indent: 0, body: line };
  });
}

function level(spaces: string): number {
  return Math.min(6, Math.floor(spaces.length / 2));
}

/** A line's words and formulas. A dollar that does not open a formula stays a dollar, as in "$10". */
export function noteSpans(body: string): NoteSpan[] {
  const spans: NoteSpan[] = [];
  let text = "";
  let index = 0;
  while (index < body.length) {
    if (body.startsWith("$$", index)) {
      const end = body.indexOf("$$", index + 2);
      if (end > index + 2) {
        if (text) spans.push({ text });
        text = "";
        spans.push({ math: body.slice(index + 2, end), block: true });
        index = end + 2;
        continue;
      }
    } else if (body[index] === "$") {
      const end = inlineDollarEnd(body.slice(index));
      if (end !== undefined) {
        if (text) spans.push({ text });
        text = "";
        spans.push({ math: body.slice(index + 1, index + end), block: false });
        index += end + 1;
        continue;
      }
    }
    text += body[index];
    index += 1;
  }
  if (text) spans.push({ text });
  return spans;
}

/** Whether a note has anything drawn other than its plain words. */
export function isFormattedNote(text: string): boolean {
  return noteLines(text).some((line) => line.kind !== "plain" || noteSpans(line.body).some((span) => "math" in span));
}

/** The text with the checkbox on line `index` ticked or cleared; unchanged if that line has none. */
export function toggleCheck(text: string, index: number): string {
  const lines = text.split("\n");
  const line = lines[index];
  const check = line === undefined ? null : CHECK.exec(line);
  if (!line || !check) return text;
  const at = line.indexOf("[") + 1;
  lines[index] = `${line.slice(0, at)}${check[2] === " " ? "x" : " "}${line.slice(at + 1)}`;
  return lines.join("\n");
}

export type ListStyle = "bullet" | "number" | "check";

/**
 * Turns the lines from `start` to `end` (offsets in `text`) into list items of
 * `style`, or back into plain lines when they all already are. Returns the new
 * text and where the selection goes.
 */
export function setListStyle(
  text: string, start: number, end: number, style: ListStyle,
): { text: string; start: number; end: number } {
  const from = text.lastIndexOf("\n", Math.max(0, start - 1)) + 1;
  const lineEnd = text.indexOf("\n", end);
  const to = lineEnd === -1 ? text.length : lineEnd;
  const lines = text.slice(from, to).split("\n");
  const parsed = noteLines(lines.join("\n"));
  const all = parsed.every((line) => line.kind === style);
  let counter = 0;
  const changed = parsed.map((line, index) => {
    const indent = "  ".repeat(line.indent);
    if (all) return `${indent}${line.body}`;
    counter += 1;
    const marker = style === "bullet" ? "- " : style === "number" ? `${counter}. ` : "[ ] ";
    const body = line.kind === "plain" ? (lines[index] ?? "").trimStart() : line.body;
    const keepIndent = line.kind === "plain" ? (lines[index] ?? "").length - (lines[index] ?? "").trimStart().length : indent.length;
    return `${" ".repeat(keepIndent)}${marker}${body}`;
  }).join("\n");
  const next = text.slice(0, from) + changed + text.slice(to);
  const shift = changed.length - (to - from);
  return { text: next, start: from, end: Math.max(from, to + shift) };
}

/**
 * Enter at `caret` on a list item: starts the next item, numbered one on, or
 * ends the list when the item is empty. Null when the line is not a list item.
 */
export function continueList(text: string, caret: number): { text: string; caret: number } | null {
  const from = text.lastIndexOf("\n", Math.max(0, caret - 1)) + 1;
  const lineEnd = text.indexOf("\n", caret);
  const line = text.slice(from, lineEnd === -1 ? text.length : lineEnd);
  const [parsed] = noteLines(line);
  if (!parsed || parsed.kind === "plain") return null;
  const indent = "  ".repeat(parsed.indent);
  if (!parsed.body.trim()) {
    // An empty item ends the list: the line is left empty.
    const to = lineEnd === -1 ? text.length : lineEnd;
    return { text: text.slice(0, from) + text.slice(to), caret: from };
  }
  const marker = parsed.kind === "bullet" ? line.trimStart().slice(0, 2)
    : parsed.kind === "check" ? (line.trimStart().startsWith("[") ? "[ ] " : `${line.trimStart().slice(0, 2)}[ ] `)
      : `${Number.parseInt(parsed.number, 10) + 1}${parsed.number.slice(-1)} `;
  const insert = `\n${indent}${marker}`;
  return { text: text.slice(0, caret) + insert + text.slice(caret), caret: caret + insert.length };
}

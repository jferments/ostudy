import type { ClozeMode } from "../../ipc";

export type ClozePart =
  | { kind: "text"; text: string }
  | { kind: "blank"; group: number; text: string };

export interface ParsedClozeMarkup {
  parts: ClozePart[];
  groups: number[];
  fullText: string;
  valid: boolean;
}

const MARKER = /\{\{c(\d+)::([\s\S]*?)\}\}/gu;

export function parseClozeMarkup(value: string): ParsedClozeMarkup {
  const parts: ClozePart[] = [];
  const groups = new Set<number>();
  let fullText = "";
  let cursor = 0;
  let valid = true;

  for (const match of value.matchAll(MARKER)) {
    const index = match.index ?? 0;
    const literal = value.slice(cursor, index);
    if (literal.includes("{{") || literal.includes("}}")) valid = false;
    if (literal !== "") {
      parts.push({ kind: "text", text: literal });
      fullText += literal;
    }
    const group = Number(match[1]);
    const text = match[2];
    if (!Number.isInteger(group) || group < 1 || text.trim() === "") valid = false;
    if (text.includes("{{") || text.includes("}}")) valid = false;
    parts.push({ kind: "blank", group, text });
    groups.add(group);
    fullText += text;
    cursor = index + match[0].length;
  }

  const tail = value.slice(cursor);
  if (tail.includes("{{") || tail.includes("}}")) valid = false;
  if (tail !== "") {
    parts.push({ kind: "text", text: tail });
    fullText += tail;
  }
  return {
    parts,
    groups: [...groups].sort((left, right) => left - right),
    fullText,
    valid,
  };
}

export function normalizeClozeMode(value: string, mode: ClozeMode): string {
  let nextGroup = 1;
  return value.replace(MARKER, (_marker, _group: string, text: string) => {
    const group = mode === "all_together" ? 1 : nextGroup++;
    return `{{c${group}::${text}}}`;
  });
}

export function clearClozeMarkers(value: string): string {
  return value.replace(MARKER, (_marker, _group: string, text: string) => text);
}

export function nextClozeGroup(value: string, mode: ClozeMode): number {
  if (mode === "all_together") return 1;
  const parsed = parseClozeMarkup(value);
  return (parsed.groups.at(-1) ?? 0) + 1;
}

export function renderClozeQuestion(
  parsed: ParsedClozeMarkup,
  group: number | null,
  mode: ClozeMode = "one_at_a_time",
): string {
  return parsed.parts
    .map((part) => {
      if (part.kind === "text") return part.text;
      return mode === "all_together" || group === null || part.group === group
        ? "[…]"
        : part.text;
    })
    .join("");
}

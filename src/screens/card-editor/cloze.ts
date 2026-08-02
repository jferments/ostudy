export interface ClozeBlank {
  group: number;
  text: string;
  start: number;
  end: number;
}

export interface ClozePreview {
  question: string;
  answer: string;
}

const CLOZE_PATTERN = /\{\{c(\d+)::([\s\S]*?)\}\}/gu;

export function findClozeBlanks(source: string): ClozeBlank[] {
  const blanks: ClozeBlank[] = [];
  for (const match of source.matchAll(CLOZE_PATTERN)) {
    const group = Number(match[1]);
    const text = (match[2] ?? "").trim();
    if (!Number.isSafeInteger(group) || group < 1 || text === "") continue;
    blanks.push({
      group,
      text,
      start: match.index ?? 0,
      end: (match.index ?? 0) + match[0].length,
    });
  }
  return blanks;
}

export function nextClozeGroup(source: string): number {
  return findClozeBlanks(source).reduce((maximum, blank) => {
    return Math.max(maximum, blank.group);
  }, 0) + 1;
}

export function markSelectionAsBlank(
  source: string,
  selectionStart: number,
  selectionEnd: number,
): { source: string; selectionStart: number; selectionEnd: number } {
  const start = Math.max(0, Math.min(selectionStart, source.length));
  const end = Math.max(start, Math.min(selectionEnd, source.length));
  const selected = source.slice(start, end);
  const leading = selected.match(/^\s*/u)?.[0] ?? "";
  const trailing = selected.match(/\s*$/u)?.[0] ?? "";
  const coreStart = leading.length;
  const coreEnd = selected.length - trailing.length;
  const core = selected.slice(coreStart, coreEnd);
  if (core === "") {
    throw new Error("Select one or more words before marking a blank.");
  }
  if (core.includes("{{") || core.includes("}}")) {
    throw new Error("The selection already contains a blank marker.");
  }

  const marker = `{{c${nextClozeGroup(source)}::${core}}}`;
  const replacement = `${leading}${marker}${trailing}`;
  const nextSource = `${source.slice(0, start)}${replacement}${source.slice(end)}`;
  const markerStart = start + leading.length;
  return {
    source: nextSource,
    selectionStart: markerStart,
    selectionEnd: markerStart + marker.length,
  };
}

export function removeAllClozeMarkers(source: string): string {
  return source.replace(CLOZE_PATTERN, (_marker, _group: string, text: string) => text);
}

export function renderClozePreview(
  source: string,
  targetGroup: number | null,
): ClozePreview {
  const answers: string[] = [];
  const question = source.replace(
    CLOZE_PATTERN,
    (_marker, rawGroup: string, rawText: string) => {
      const group = Number(rawGroup);
      const text = rawText.trim();
      if (targetGroup === null || targetGroup === group) {
        answers.push(text);
        return "[…]";
      }
      return text;
    },
  );
  const completeStatement = removeAllClozeMarkers(source);
  return {
    question,
    answer:
      answers.length === 0
        ? "Mark at least one blank to preview this card."
        : `${answers.join(" · ")}\n\nComplete statement: ${completeStatement}`,
  };
}

export function distinctClozeGroups(source: string): number[] {
  return [...new Set(findClozeBlanks(source).map((blank) => blank.group))].sort(
    (left, right) => left - right,
  );
}

/**
 * Quizlet's Export gives a set as text: each card's term and definition joined
 * by a tab or a comma, and cards joined by new lines or semicolons. Nothing
 * here writes; the parsed rows go through the native bulk-card validation.
 */
export type QuizletTermSeparator = "tab" | "comma";
export type QuizletRowSeparator = "newline" | "semicolon";

export interface QuizletSeparators {
  term: QuizletTermSeparator;
  row: QuizletRowSeparator;
}

export interface QuizletCard {
  question: string;
  answer: string;
}

export interface ParsedQuizletText {
  cards: QuizletCard[];
  /** Non-empty rows without both a term and a definition. */
  skipped: number;
}

/** Quizlet's own defaults are a tab and a new line; anything else is read from the text. */
export function detectQuizletSeparators(text: string): QuizletSeparators {
  const term: QuizletTermSeparator = text.includes("\t") || !text.includes(",") ? "tab" : "comma";
  const lines = text.split(/\r?\n/u).filter((line) => line.trim() !== "");
  const row: QuizletRowSeparator = lines.length <= 1 && text.includes(";") ? "semicolon" : "newline";
  return { term, row };
}

export function parseQuizletText(text: string, separators: QuizletSeparators): ParsedQuizletText {
  const rows = separators.row === "newline" ? text.split(/\r?\n/u) : text.split(";");
  const divider = separators.term === "tab" ? "\t" : ",";
  const cards: QuizletCard[] = [];
  let skipped = 0;
  for (const row of rows) {
    if (row.trim() === "") continue;
    const at = row.indexOf(divider);
    const question = at < 0 ? row.trim() : row.slice(0, at).trim();
    const answer = at < 0 ? "" : row.slice(at + divider.length).trim();
    if (question === "" || answer === "") {
      skipped += 1;
      continue;
    }
    cards.push({ question, answer });
  }
  return { cards, skipped };
}

import type { CardNote } from "./cards";
import type { QuestionNote } from "./questions";

export type AuthoringNote = CardNote | QuestionNote;

export function isQuestionNote(note: AuthoringNote): note is QuestionNote {
  return "definition" in note;
}

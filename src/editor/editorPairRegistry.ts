import type { RichDocument } from "../ipc/common";
import type { EditorAdapter } from "./adapter";
import {
  copyAnswerKeyChecklistsToAnswer,
  extractRichDocumentText,
  hasAnswerKeyChecklist,
  validateAnswerChecklistPair,
} from "./richText";

export type CardEditorSide = "question" | "answer";

interface PairMember {
  adapter: EditorAdapter;
  apply(document: RichDocument, plainText: string): void;
}

interface PairState {
  question?: PairMember;
  answer?: PairMember;
  listeners: Set<() => void>;
}

const pairs = new Map<string, PairState>();

export function registerCardEditorPairMember(
  pairId: string,
  side: CardEditorSide,
  member: PairMember,
): () => void {
  const state = pairs.get(pairId) ?? { listeners: new Set<() => void>() };
  state[side] = member;
  pairs.set(pairId, state);
  notify(state);
  return () => {
    const current = pairs.get(pairId);
    if (!current || current[side] !== member) return;
    delete current[side];
    notify(current);
    if (!current.question && !current.answer && current.listeners.size === 0) pairs.delete(pairId);
  };
}

export function subscribeCardEditorPair(pairId: string, listener: () => void): () => void {
  const state = pairs.get(pairId) ?? { listeners: new Set<() => void>() };
  state.listeners.add(listener);
  pairs.set(pairId, state);
  return () => {
    const current = pairs.get(pairId);
    current?.listeners.delete(listener);
    if (current && !current.question && !current.answer && current.listeners.size === 0) {
      pairs.delete(pairId);
    }
  };
}

export function cardEditorPairCanCopy(pairId: string): boolean {
  const state = pairs.get(pairId);
  return Boolean(
    state?.question &&
      state.answer &&
      hasAnswerKeyChecklist(state.question.adapter.document()),
  );
}

export function copyCardEditorChecklistToAnswer(pairId: string): string | null {
  const state = pairs.get(pairId);
  if (!state?.question || !state.answer) {
    return "The question and answer editors are still opening.";
  }
  const copied = copyAnswerKeyChecklistsToAnswer(
    state.question.adapter.document(),
    state.answer.adapter.document(),
  );
  const validation = validateAnswerChecklistPair(copied.question, copied.answer);
  if (validation) return validation;

  state.question.apply(copied.question, extractRichDocumentText(copied.question));
  state.answer.apply(copied.answer, extractRichDocumentText(copied.answer));
  notify(state);
  return null;
}

function notify(state: PairState): void {
  for (const listener of state.listeners) listener();
}

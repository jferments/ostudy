import type { RichDocument } from "../../ipc/common";
import type {
  ChoiceOption,
  MatchingPair,
  QuestionDefinition,
  QuestionKind,
  QuestionNote,
} from "../../ipc/questions";
import { EMPTY_RICH_DOCUMENT, cloneDocument, collectAssetIds, isDocumentEmpty } from "../../editor/richText";

export type QuestionEditorKind = Extract<
  QuestionKind,
  "single_choice" | "select_all" | "true_false" | "matching"
>;

export interface DraftRichField {
  id: string;
  doc: RichDocument;
  text: string;
}

export interface DraftChoiceOption extends DraftRichField {
  rationale: RichDocument;
  rationaleText: string;
}

export type QuestionEditorDraft =
  | {
      kind: "single_choice" | "select_all";
      prompt: RichDocument;
      promptText: string;
      explanation: RichDocument;
      explanationText: string;
      options: DraftChoiceOption[];
      correctOptionIds: string[];
      emptySetConfirmed: boolean;
    }
  | {
      kind: "true_false";
      statement: RichDocument;
      statementText: string;
      correct: boolean | null;
      explanation: RichDocument;
      explanationText: string;
      correctedStatement: RichDocument;
      correctedStatementText: string;
    }
  | {
      kind: "matching";
      prompt: RichDocument;
      promptText: string;
      explanation: RichDocument;
      explanationText: string;
      pairs: Array<{
        id: string;
        left: DraftRichField;
        right: DraftRichField;
        rationale: RichDocument;
        rationaleText: string;
      }>;
    };

export function isQuestionEditorKind(value: string): value is QuestionEditorKind {
  return ["single_choice", "select_all", "true_false", "matching"].includes(value);
}

export function questionKindForNote(note: QuestionNote): QuestionEditorKind | null {
  const kind = note.definition.question.kind;
  return isQuestionEditorKind(kind) ? kind : null;
}

export function newQuestionDraft(kind: QuestionEditorKind): QuestionEditorDraft {
  if (kind === "true_false") {
    return {
      kind,
      statement: emptyDoc(),
      statementText: "",
      correct: null,
      explanation: emptyDoc(),
      explanationText: "",
      correctedStatement: emptyDoc(),
      correctedStatementText: "",
    };
  }
  if (kind === "matching") {
    return {
      kind,
      prompt: emptyDoc(),
      promptText: "",
      explanation: emptyDoc(),
      explanationText: "",
      pairs: [newPair(), newPair()],
    };
  }
  return {
    kind,
    prompt: emptyDoc(),
    promptText: "",
    explanation: emptyDoc(),
    explanationText: "",
    options: [newChoiceOption(), newChoiceOption()],
    correctOptionIds: [],
    emptySetConfirmed: false,
  };
}

export function draftFromQuestionNote(note: QuestionNote): QuestionEditorDraft | null {
  const question = note.definition.question;
  if (question.kind === "single_choice" || question.kind === "select_all") {
    const correctOptionIds = question.kind === "single_choice"
      ? [question.correctOptionId]
      : [...question.correctOptionIds];
    return {
      kind: question.kind,
      prompt: cloneDocument(question.prompt),
      promptText: plainText(question.prompt),
      explanation: cloneDocument(question.explanation ?? EMPTY_RICH_DOCUMENT),
      explanationText: plainText(question.explanation),
      options: question.options.map((option) => ({
        id: option.id,
        doc: cloneDocument(option.content),
        text: plainText(option.content),
        rationale: cloneDocument(option.rationale ?? EMPTY_RICH_DOCUMENT),
        rationaleText: plainText(option.rationale),
      })),
      correctOptionIds,
      emptySetConfirmed: question.kind === "select_all" && question.correctOptionIds.length === 0,
    };
  }
  if (question.kind === "true_false") {
    return {
      kind: question.kind,
      statement: cloneDocument(question.statement),
      statementText: plainText(question.statement),
      correct: question.correct,
      explanation: cloneDocument(question.explanation ?? EMPTY_RICH_DOCUMENT),
      explanationText: plainText(question.explanation),
      correctedStatement: cloneDocument(question.correctedStatement ?? EMPTY_RICH_DOCUMENT),
      correctedStatementText: plainText(question.correctedStatement),
    };
  }
  if (question.kind === "matching") {
    return {
      kind: question.kind,
      prompt: cloneDocument(question.prompt),
      promptText: plainText(question.prompt),
      explanation: cloneDocument(question.explanation ?? EMPTY_RICH_DOCUMENT),
      explanationText: plainText(question.explanation),
      pairs: question.pairs.map((pair) => ({
        id: pair.left.id,
        left: { id: pair.left.id, doc: cloneDocument(pair.left.content), text: plainText(pair.left.content) },
        right: { id: pair.right.id, doc: cloneDocument(pair.right.content), text: plainText(pair.right.content) },
        rationale: cloneDocument(pair.rationale ?? EMPTY_RICH_DOCUMENT),
        rationaleText: plainText(pair.rationale),
      })),
    };
  }
  return null;
}

export function questionDraftReady(draft: QuestionEditorDraft): boolean {
  if (draft.kind === "true_false") {
    return draft.correct !== null && !isDocumentEmpty(draft.statementText, draft.statement);
  }
  if (draft.kind === "matching") {
    return draft.pairs.length >= 2 && draft.pairs.length <= 16
      && !isDocumentEmpty(draft.promptText, draft.prompt)
      && draft.pairs.every((pair) =>
        !isDocumentEmpty(pair.left.text, pair.left.doc)
        && !isDocumentEmpty(pair.right.text, pair.right.doc));
  }
  const keyReady = draft.kind === "single_choice"
    ? draft.correctOptionIds.length === 1
    : draft.correctOptionIds.length > 0 || draft.emptySetConfirmed;
  return keyReady
    && draft.options.length >= 2
    && draft.options.length <= 32
    && !isDocumentEmpty(draft.promptText, draft.prompt)
    && draft.options.every((option) => !isDocumentEmpty(option.text, option.doc));
}

export function definitionFromDraft(draft: QuestionEditorDraft): QuestionDefinition {
  if (draft.kind === "true_false") {
    if (draft.correct === null) throw new Error("Choose whether the statement is true or false.");
    return {
      schemaVersion: 1,
      question: {
        kind: draft.kind,
        statement: draft.statement,
        correct: draft.correct,
        explanation: optionalDocument(draft.explanationText, draft.explanation),
        correctedStatement: optionalDocument(draft.correctedStatementText, draft.correctedStatement),
      },
    };
  }
  if (draft.kind === "matching") {
    const pairs: MatchingPair[] = draft.pairs.map((pair) => ({
      left: { id: pair.left.id, content: pair.left.doc },
      right: { id: pair.right.id, content: pair.right.doc },
      rationale: optionalDocument(pair.rationaleText, pair.rationale),
    }));
    return {
      schemaVersion: 1,
      question: {
        kind: draft.kind,
        prompt: draft.prompt,
        pairs,
        explanation: optionalDocument(draft.explanationText, draft.explanation),
      },
    };
  }
  const options: ChoiceOption[] = draft.options.map((option) => ({
    id: option.id,
    content: option.doc,
    rationale: optionalDocument(option.rationaleText, option.rationale),
  }));
  if (draft.kind === "single_choice") {
    const correctOptionId = draft.correctOptionIds[0];
    if (!correctOptionId) throw new Error("Choose the single best answer.");
    return {
      schemaVersion: 1,
      question: {
        kind: draft.kind,
        prompt: draft.prompt,
        options,
        correctOptionId,
        explanation: optionalDocument(draft.explanationText, draft.explanation),
      },
    };
  }
  return {
    schemaVersion: 1,
    question: {
      kind: draft.kind,
      prompt: draft.prompt,
      options,
      correctOptionIds: [...draft.correctOptionIds],
      explanation: optionalDocument(draft.explanationText, draft.explanation),
    },
  };
}

export function collectQuestionDraftAssetIds(draft: QuestionEditorDraft): string[] {
  const ids = new Set<string>();
  const add = (document: RichDocument): void => {
    for (const id of collectAssetIds(document)) ids.add(id);
  };
  if (draft.kind === "true_false") {
    add(draft.statement);
    add(draft.explanation);
    add(draft.correctedStatement);
  } else if (draft.kind === "matching") {
    add(draft.prompt);
    add(draft.explanation);
    for (const pair of draft.pairs) {
      add(pair.left.doc);
      add(pair.right.doc);
      add(pair.rationale);
    }
  } else {
    add(draft.prompt);
    add(draft.explanation);
    for (const option of draft.options) {
      add(option.doc);
      add(option.rationale);
    }
  }
  return [...ids].sort();
}

export function updateChoiceOption(
  draft: Extract<QuestionEditorDraft, { kind: "single_choice" | "select_all" }>,
  optionId: string,
  update: Partial<DraftChoiceOption>,
): QuestionEditorDraft {
  return {
    ...draft,
    options: draft.options.map((option) => option.id === optionId ? { ...option, ...update } : option),
  };
}

export function addChoiceOption(
  draft: Extract<QuestionEditorDraft, { kind: "single_choice" | "select_all" }>,
): QuestionEditorDraft {
  if (draft.options.length >= 32) return draft;
  return { ...draft, options: [...draft.options, newChoiceOption()] };
}

export function removeChoiceOption(
  draft: Extract<QuestionEditorDraft, { kind: "single_choice" | "select_all" }>,
  optionId: string,
): QuestionEditorDraft {
  if (draft.options.length <= 2) return draft;
  return {
    ...draft,
    options: draft.options.filter((option) => option.id !== optionId),
    correctOptionIds: draft.correctOptionIds.filter((id) => id !== optionId),
  };
}

export function moveChoiceOption(
  draft: Extract<QuestionEditorDraft, { kind: "single_choice" | "select_all" }>,
  optionId: string,
  delta: -1 | 1,
): QuestionEditorDraft {
  const index = draft.options.findIndex((option) => option.id === optionId);
  const target = index + delta;
  if (index < 0 || target < 0 || target >= draft.options.length) return draft;
  const options = [...draft.options];
  [options[index], options[target]] = [options[target], options[index]];
  return { ...draft, options };
}

export function addMatchingPair(
  draft: Extract<QuestionEditorDraft, { kind: "matching" }>,
): QuestionEditorDraft {
  if (draft.pairs.length >= 16) return draft;
  return { ...draft, pairs: [...draft.pairs, newPair()] };
}

export function removeMatchingPair(
  draft: Extract<QuestionEditorDraft, { kind: "matching" }>,
  pairId: string,
): QuestionEditorDraft {
  if (draft.pairs.length <= 2) return draft;
  return { ...draft, pairs: draft.pairs.filter((pair) => pair.id !== pairId) };
}

export function moveMatchingPair(
  draft: Extract<QuestionEditorDraft, { kind: "matching" }>,
  pairId: string,
  delta: -1 | 1,
): QuestionEditorDraft {
  const index = draft.pairs.findIndex((pair) => pair.id === pairId);
  const target = index + delta;
  if (index < 0 || target < 0 || target >= draft.pairs.length) return draft;
  const pairs = [...draft.pairs];
  [pairs[index], pairs[target]] = [pairs[target], pairs[index]];
  return { ...draft, pairs };
}

function newChoiceOption(): DraftChoiceOption {
  return {
    id: newEntityId(),
    doc: emptyDoc(),
    text: "",
    rationale: emptyDoc(),
    rationaleText: "",
  };
}

function newPair() {
  const leftId = newEntityId();
  return {
    id: leftId,
    left: { id: leftId, doc: emptyDoc(), text: "" },
    right: { id: newEntityId(), doc: emptyDoc(), text: "" },
    rationale: emptyDoc(),
    rationaleText: "",
  };
}

function emptyDoc(): RichDocument {
  return cloneDocument(EMPTY_RICH_DOCUMENT);
}

function optionalDocument(text: string, document: RichDocument): RichDocument | null {
  return isDocumentEmpty(text, document) ? null : document;
}

function plainText(document: RichDocument | null): string {
  if (!document) return "";
  const parts: string[] = [];
  const pending: unknown[] = [document];
  while (pending.length > 0) {
    const current = pending.pop();
    if (!current || typeof current !== "object") continue;
    const node = current as { text?: unknown; content?: unknown };
    if (typeof node.text === "string") parts.push(node.text);
    if (Array.isArray(node.content)) pending.push(...[...node.content].reverse());
  }
  return parts.join(" ").replace(/\s+/gu, " ").trim();
}

function newEntityId(): string {
  return crypto.randomUUID().replaceAll("-", "");
}

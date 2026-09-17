import type { PublicQuestion } from "../../ipc/questionAttempts";
import type { QuestionEditorDraft } from "./questionDraft";

export function publicQuestionFromDraft(draft: QuestionEditorDraft): PublicQuestion | null {
  switch (draft.kind) {
    case "single_choice":
    case "select_all":
      return {
        kind: draft.kind,
        prompt: draft.prompt,
        options: previewOrder(draft.options.map((option) => ({ id: option.id, content: option.doc }))),
      };
    case "true_false":
      return { kind: draft.kind, statement: draft.statement };
    case "matching":
      return {
        kind: draft.kind,
        prompt: draft.prompt,
        items: draft.pairs.map((pair) => ({ id: pair.left.id, content: pair.left.doc })),
        targets: previewOrder(draft.pairs.map((pair) => ({ id: pair.right.id, content: pair.right.doc }))),
      };
    case "grouping":
      return {
        kind: draft.kind,
        prompt: draft.prompt,
        categories: draft.categories.map((category) => ({ id: category.id, content: category.doc })),
        items: previewOrder(draft.items.map((item) => ({ id: item.id, content: item.doc }))),
      };
    case "ordering":
      return {
        kind: draft.kind,
        prompt: draft.prompt,
        items: previewOrder(draft.items.map((item) => ({ id: item.id, content: item.doc }))),
      };
    case "image_occlusion":
      return null;
  }
}

export function questionPreviewStructureKey(question: PublicQuestion): string {
  switch (question.kind) {
    case "single_choice":
    case "select_all":
      return `${question.kind}:${question.options.map((item) => item.id).join(",")}`;
    case "true_false":
      return question.kind;
    case "matching":
      return `${question.kind}:${question.items.map((item) => item.id).join(",")}:${question.targets.map((item) => item.id).join(",")}`;
    case "grouping":
      return `${question.kind}:${question.items.map((item) => item.id).join(",")}:${question.categories.map((item) => item.id).join(",")}`;
    case "ordering":
      return `${question.kind}:${question.items.map((item) => item.id).join(",")}`;
  }
}

function previewOrder<T>(items: T[]): T[] {
  if (items.length < 2) return items;
  return [...items.slice(1), items[0]];
}

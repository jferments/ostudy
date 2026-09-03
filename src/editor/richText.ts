import {
  type Asset,
} from "../ipc/assets";
import { managedAssetUrl } from "../platform/assets";
import {
  type RichDocument,
} from "../ipc/common";
import { RICH_DOCUMENT_SCHEMA_VERSION } from "./adapter";
import { ANSWER_KEY_MODE } from "./extensions";

export const EMPTY_RICH_DOCUMENT: RichDocument = {
  type: "doc",
  attrs: { schemaVersion: RICH_DOCUMENT_SCHEMA_VERSION },
  content: [{ type: "paragraph" }],
};

export type AnswerKeySide = "question" | "answer";

export interface CopiedAnswerChecklist {
  question: RichDocument;
  answer: RichDocument;
}

export function cloneDocument(document: RichDocument): RichDocument {
  return structuredClone(document);
}

export function canonicalizeDocument(document: RichDocument): RichDocument {
  const copy = cloneDocument(document);
  copy.type = "doc";
  copy.attrs = {
    ...(isRecord(copy.attrs) ? copy.attrs : {}),
    schemaVersion: RICH_DOCUMENT_SCHEMA_VERSION,
  };
  walkMutable(copy, (node) => {
    if (node.type !== "image" || !isRecord(node.attrs)) {
      return;
    }
    const assetId = node.attrs.assetId;
    if (typeof assetId === "string" && assetId !== "") {
      node.attrs.src = `study-asset://${assetId}`;
    }
    for (const key of ["width", "height"] as const) {
      const value = node.attrs[key];
      const numeric =
        typeof value === "number"
          ? value
          : typeof value === "string" && value.trim() !== ""
            ? Number(value)
            : Number.NaN;

      node.attrs[key] =
        Number.isFinite(numeric) && numeric >= 1 && numeric <= 10_000
          ? numeric
          : null;
    }
  });
  return copy;
}

export function hydrateDocument(
  document: RichDocument,
  assets: readonly Asset[],
): RichDocument {
  const copy = cloneDocument(document);
  const byId = new Map(assets.map((asset) => [asset.id, asset]));

  walkMutable(copy, (node) => {
    if (node.type !== "image" || !isRecord(node.attrs)) {
      return;
    }
    const assetId = node.attrs.assetId;
    if (typeof assetId !== "string") {
      return;
    }
    const asset = byId.get(assetId);
    if (asset) {
      node.attrs.src = managedAssetUrl(asset.id);
      if (typeof node.attrs.alt !== "string" || node.attrs.alt.trim() === "") {
        node.attrs.alt = asset.originalFilename;
      }
    }
  });

  return copy;
}

export function collectAssetIds(document: RichDocument): string[] {
  const ids = new Set<string>();
  walk(document, (node) => {
    if (node.type !== "image" || !isRecord(node.attrs)) {
      return;
    }
    const assetId = node.attrs.assetId;
    if (typeof assetId === "string" && assetId !== "") {
      ids.add(assetId);
    }
  });
  return [...ids];
}

export function isDocumentEmpty(
  text: string,
  document: RichDocument,
): boolean {
  return text.trim() === "" && collectAssetIds(document).length === 0;
}

export function hasAnswerKeyChecklist(document: RichDocument): boolean {
  return answerKeyLists(document).length > 0;
}

/**
 * Copies every answer-key list from the question into the answer while keeping
 * stable item identities. The marked key is copied first, then the persisted
 * question is cleared so practice can conceal correctness without relying on a
 * presentation-only mutation. Existing answer-key lists are replaced rather
 * than appended, which makes repeated use idempotent.
 */
export function copyAnswerKeyChecklistsToAnswer(
  questionDocument: RichDocument,
  answerDocument: RichDocument,
): CopiedAnswerChecklist {
  const question = canonicalizeDocument(questionDocument);
  const seen = new Set<string>();
  const lists: Record<string, unknown>[] = [];
  walkMutable(question, (node) => {
    if (!isAnswerKeyList(node)) return;
    for (const item of nodeContent(node)) {
      if (!isRecord(item) || item.type !== "taskItem") continue;
      const attrs = isRecord(item.attrs) ? item.attrs : {};
      let itemId = typeof attrs.itemId === "string" ? attrs.itemId : "";
      if (!/^[0-9a-f]{32}$/u.test(itemId) || seen.has(itemId)) {
        itemId = createChecklistItemId();
      }
      seen.add(itemId);
      item.attrs = { ...attrs, itemId };
    }

    // Preserve the author's marked key on the answer before clearing the
    // question-side boxes required by the durable schema contract.
    lists.push(cloneRecord(node));
    for (const item of nodeContent(node)) {
      if (!isRecord(item) || item.type !== "taskItem") continue;
      const attrs = isRecord(item.attrs) ? item.attrs : {};
      item.attrs = { ...attrs, checked: false };
    }
  });

  const answer = canonicalizeDocument(answerDocument);
  const answerContent = nodeContent(answer).filter(
    (node) => !isRecord(node) || !isAnswerKeyList(node),
  );
  const meaningfulContent = answerContent.filter(
    (node) => !isEmptyParagraph(node),
  );
  answer.content = [...meaningfulContent, ...lists.map(cloneRecord)];
  if ((answer.content as unknown[]).length === 0) {
    answer.content = [{ type: "paragraph" }];
  }

  return { question, answer };
}

/** Returns null when the question/answer checklist pair is safe to save. */
export function validateAnswerChecklistPair(
  questionDocument: RichDocument,
  answerDocument: RichDocument,
): string | null {
  const questionLists = checklistIdentities(questionDocument);
  if (questionLists.length === 0) return null;
  const answerLists = checklistIdentities(answerDocument);

  for (const list of questionLists) {
    if (list.missing) {
      return "Every answer-checklist item needs a stable identity. Use Copy checklist to answer to repair it.";
    }
    if (list.duplicate) {
      return "Answer-checklist item identities must be unique. Use Copy checklist to answer to repair them.";
    }
    if (list.checked.some(Boolean)) {
      return "Question answer-checklist boxes must be empty. Use Copy checklist to answer after marking the key.";
    }
  }
  if (answerLists.length !== questionLists.length) {
    return "Copy the question checklist to the answer before saving this card.";
  }
  for (let index = 0; index < questionLists.length; index += 1) {
    const question = questionLists[index];
    const answer = answerLists[index];
    if (!question || !answer || question.ids.join(",") !== answer.ids.join(",")) {
      return "The question and answer checklists no longer contain the same items. Copy the checklist to the answer again.";
    }
    if (answer.missing || answer.duplicate) {
      return "The answer checklist has invalid item identities. Copy the checklist to the answer again.";
    }
    if (!answer.checked.some(Boolean)) {
      return "Mark at least one correct answer before copying the checklist to the answer.";
    }
  }
  return null;
}

/**
 * Read-only presentation transform. Ordinary task lists retain their authored
 * state. Answer-key lists conceal correctness on the question and reveal it on
 * the answer without changing the persisted document.
 */
export function prepareDocumentForPresentation(
  document: RichDocument,
  side: AnswerKeySide | null,
): RichDocument {
  const copy = cloneDocument(document);
  if (!side) return copy;
  visitLists(copy, (list) => {
    if (!isAnswerKeyList(list)) return;
    for (const item of nodeContent(list)) {
      if (!isRecord(item) || item.type !== "taskItem") continue;
      const attrs = isRecord(item.attrs) ? item.attrs : {};
      item.attrs = {
        ...attrs,
        checked: side === "answer" ? attrs.checked === true : false,
      };
    }
  });
  return copy;
}

export function extractRichDocumentText(document: RichDocument): string {
  const parts: string[] = [];
  extract(document, parts, null);
  return parts
    .join("")
    .replace(/[ \t]+\n/gu, "\n")
    .replace(/\n{3,}/gu, "\n\n")
    .trim();
}

function extract(value: unknown, parts: string[], parentType: string | null): void {
  if (Array.isArray(value)) {
    value.forEach((entry, index) => {
      extract(entry, parts, parentType);
      if (parentType === "tableRow" && index < value.length - 1) parts.push("\t");
    });
    return;
  }
  if (!isRecord(value)) return;
  const type = typeof value.type === "string" ? value.type : "";
  if (type === "text" && typeof value.text === "string") {
    parts.push(value.text);
    return;
  }
  if (type === "hardBreak") {
    parts.push("\n");
    return;
  }
  if (type === "taskItem") {
    const attrs = isRecord(value.attrs) ? value.attrs : {};
    parts.push(attrs.checked === true ? "[x] " : "[ ] ");
  }
  extract(value.content, parts, type);
  if (
    [
      "paragraph",
      "heading",
      "blockquote",
      "listItem",
      "taskItem",
      "codeBlock",
      "tableRow",
      "table",
    ].includes(type)
  ) {
    if (parts.length > 0 && parts.at(-1) !== "\n") parts.push("\n");
  }
}

interface ChecklistIdentity {
  ids: string[];
  checked: boolean[];
  missing: boolean;
  duplicate: boolean;
}

function checklistIdentities(document: RichDocument): ChecklistIdentity[] {
  return answerKeyLists(document).map((list) => {
    const ids: string[] = [];
    const checked: boolean[] = [];
    let missing = false;
    let duplicate = false;
    const seen = new Set<string>();
    for (const item of nodeContent(list)) {
      if (!isRecord(item) || item.type !== "taskItem") continue;
      const attrs = isRecord(item.attrs) ? item.attrs : {};
      const id = typeof attrs.itemId === "string" ? attrs.itemId : "";
      if (!/^[0-9a-f]{32}$/u.test(id)) missing = true;
      if (id !== "" && seen.has(id)) duplicate = true;
      if (id !== "") seen.add(id);
      ids.push(id);
      checked.push(attrs.checked === true);
    }
    return { ids, checked, missing, duplicate };
  });
}

function answerKeyLists(document: RichDocument): Record<string, unknown>[] {
  const lists: Record<string, unknown>[] = [];
  visitLists(document, (list) => {
    if (isAnswerKeyList(list)) lists.push(list);
  });
  return lists;
}

function visitLists(value: unknown, visitor: (node: Record<string, unknown>) => void): void {
  if (Array.isArray(value)) {
    for (const item of value) visitLists(item, visitor);
    return;
  }
  if (!isRecord(value)) return;
  if (value.type === "taskList") visitor(value);
  for (const nested of Object.values(value)) visitLists(nested, visitor);
}

function isAnswerKeyList(node: Record<string, unknown>): boolean {
  return (
    node.type === "taskList" &&
    isRecord(node.attrs) &&
    node.attrs.mode === ANSWER_KEY_MODE
  );
}

function nodeContent(node: Record<string, unknown>): unknown[] {
  return Array.isArray(node.content) ? node.content : [];
}

function isEmptyParagraph(value: unknown): boolean {
  return (
    isRecord(value) &&
    value.type === "paragraph" &&
    (!Array.isArray(value.content) || value.content.length === 0)
  );
}

function createChecklistItemId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID().replaceAll("-", "");
  }
  const random = Math.random().toString(16).slice(2).padEnd(32, "0");
  return random.slice(0, 32);
}

function cloneRecord(value: Record<string, unknown>): Record<string, unknown> {
  return structuredClone(value);
}

function walk(value: unknown, visitor: (record: Record<string, unknown>) => void) {
  if (Array.isArray(value)) {
    for (const item of value) {
      walk(item, visitor);
    }
    return;
  }
  if (!isRecord(value)) {
    return;
  }
  visitor(value);
  for (const nested of Object.values(value)) {
    walk(nested, visitor);
  }
}

function walkMutable(
  value: unknown,
  visitor: (record: Record<string, unknown>) => void,
) {
  if (Array.isArray(value)) {
    for (const item of value) {
      walkMutable(item, visitor);
    }
    return;
  }
  if (!isRecord(value)) {
    return;
  }
  visitor(value);
  for (const nested of Object.values(value)) {
    walkMutable(nested, visitor);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

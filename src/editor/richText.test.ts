import { beforeEach, describe, expect, it, vi } from "vitest";

import { managedAssetUrl } from "../platform/assets";
import { makeAsset, paragraphDocument } from "../test/factories";
import {
  canonicalizeDocument,
  cloneDocument,
  collectAssetIds,
  copyAnswerKeyChecklistsToAnswer,
  extractRichDocumentText,
  hasAnswerKeyChecklist,
  hydrateDocument,
  isDocumentEmpty,
  prepareDocumentForPresentation,
  validateAnswerChecklistPair,
} from "./richText";

vi.mock("../platform/assets", () => ({
  managedAssetUrl: vi.fn((assetId: string) => `asset-url:${assetId}`),
}));

describe("rich document helpers", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("canonicalizes managed images and schema version without mutating editor state", () => {
    const source = {
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            assetId: "a".repeat(32),
            src: "asset-url:temporary",
            width: "640",
            height: 20_000,
          },
        },
      ],
    };

    const canonical = canonicalizeDocument(source);

    expect(canonical).not.toBe(source);
    expect(canonical).toEqual({
      type: "doc",
      attrs: { schemaVersion: 2 },
      content: [
        {
          type: "image",
          attrs: {
            assetId: "a".repeat(32),
            src: `study-asset://${"a".repeat(32)}`,
            width: 640,
            height: null,
          },
        },
      ],
    });
    expect(source.content[0]?.attrs.src).toBe("asset-url:temporary");
  });

  it("hydrates only known managed assets and supplies missing alt text", () => {
    const asset = makeAsset({
      id: "b".repeat(32),
      originalFilename: "heart.png",
    });
    const document = {
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            assetId: asset.id,
            src: `study-asset://${asset.id}`,
            alt: "",
          },
        },
        {
          type: "image",
          attrs: {
            assetId: "c".repeat(32),
            src: `study-asset://${"c".repeat(32)}`,
            alt: "Unresolved",
          },
        },
      ],
    };

    const hydrated = hydrateDocument(document, [asset]);

    expect(managedAssetUrl).toHaveBeenCalledWith(asset.id);
    expect(hydrated).toEqual({
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            assetId: asset.id,
            src: `asset-url:${asset.id}`,
            alt: asset.originalFilename,
          },
        },
        {
          type: "image",
          attrs: {
            assetId: "c".repeat(32),
            src: `study-asset://${"c".repeat(32)}`,
            alt: "Unresolved",
          },
        },
      ],
    });
  });

  it("collects unique image identities from nested documents", () => {
    const first = "d".repeat(32);
    const second = "e".repeat(32);
    expect(
      collectAssetIds({
        type: "doc",
        content: [
          { type: "image", attrs: { assetId: first } },
          {
            type: "blockquote",
            content: [
              { type: "image", attrs: { assetId: second } },
              { type: "image", attrs: { assetId: first } },
            ],
          },
        ],
      }),
    ).toEqual([first, second]);
  });

  it("treats image-only documents as non-empty", () => {
    expect(isDocumentEmpty("", paragraphDocument(""))).toBe(true);
    expect(
      isDocumentEmpty("", {
        type: "doc",
        content: [{ type: "image", attrs: { assetId: "f".repeat(32) } }],
      }),
    ).toBe(false);
    expect(isDocumentEmpty("Visible text", paragraphDocument("Visible text"))).toBe(false);
  });

  it("deep-clones a document before callers mutate it", () => {
    const original = paragraphDocument("Original");
    const clone = cloneDocument(original);
    const paragraph = (clone.content as Array<Record<string, unknown>>)[0];
    const text = (paragraph?.content as Array<Record<string, unknown>>)[0];
    if (text) text.text = "Changed";

    expect(original).toEqual(paragraphDocument("Original"));
    expect(clone).toEqual(paragraphDocument("Changed"));
  });

  it("copies answer-key checklists with stable matching item identities", () => {
    const first = "1".repeat(32);
    const second = "2".repeat(32);
    const question = answerChecklist([
      { id: first, text: "Check blood pressure", correct: true },
      { id: second, text: "Give a contraindicated drug", correct: false },
    ]);

    const copied = copyAnswerKeyChecklistsToAnswer(question, paragraphDocument("Rationale"));

    expect(hasAnswerKeyChecklist(copied.question)).toBe(true);
    expect(validateAnswerChecklistPair(copied.question, copied.answer)).toBeNull();
    expect(extractRichDocumentText(copied.answer)).toContain("[x] Check blood pressure");
    expect(extractRichDocumentText(copied.answer)).toContain("[ ] Give a contraindicated drug");
  });

  it("repairs missing or duplicate checklist identities only through the copy action", () => {
    const question = answerChecklist([
      { id: "", text: "First", correct: true },
      { id: "3".repeat(32), text: "Second", correct: false },
      { id: "3".repeat(32), text: "Third", correct: true },
    ]);
    expect(validateAnswerChecklistPair(question, paragraphDocument(""))).toMatch(/stable identity/u);

    const copied = copyAnswerKeyChecklistsToAnswer(question, paragraphDocument(""));
    expect(validateAnswerChecklistPair(copied.question, copied.answer)).toBeNull();
  });

  it("conceals answer-key state on questions and preserves ordinary task state", () => {
    const answerKey = answerChecklist([
      { id: "4".repeat(32), text: "Correct", correct: true },
    ]);
    const question = prepareDocumentForPresentation(answerKey, "question");
    const answer = prepareDocumentForPresentation(answerKey, "answer");

    expect(extractRichDocumentText(question)).toContain("[ ] Correct");
    expect(extractRichDocumentText(answer)).toContain("[x] Correct");

    const ordinary = taskList(true);
    expect(extractRichDocumentText(prepareDocumentForPresentation(ordinary, "question"))).toContain(
      "[x] Ordinary",
    );
  });
});

function answerChecklist(
  entries: Array<{ id: string; text: string; correct: boolean }>,
) {
  return {
    type: "doc",
    attrs: { schemaVersion: 2 },
    content: [
      {
        type: "taskList",
        attrs: { mode: "answer_key" },
        content: entries.map((entry) => ({
          type: "taskItem",
          attrs: { itemId: entry.id, checked: entry.correct },
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: entry.text }],
            },
          ],
        })),
      },
    ],
  };
}

function taskList(checked: boolean) {
  return {
    type: "doc",
    attrs: { schemaVersion: 2 },
    content: [
      {
        type: "taskList",
        attrs: { mode: "ordinary" },
        content: [
          {
            type: "taskItem",
            attrs: { itemId: null, checked },
            content: [
              { type: "paragraph", content: [{ type: "text", text: "Ordinary" }] },
            ],
          },
        ],
      },
    ],
  };
}

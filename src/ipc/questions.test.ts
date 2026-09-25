import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getQuestionNote,
  previewQuestionDraft,
  saveQuestionNote,
  type SaveQuestionNoteInput,
} from "./questions";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

beforeEach(() => {
  invoke.mockReset();
});

describe("interactive question authoring gateway", () => {
  it("keeps author preview on the pure native preview command", async () => {
    const input = {
      definitionJson: "{\"schemaVersion\":1}",
      response: { kind: "true_false" as const, selected: true },
    };
    const result = {
      question: { kind: "true_false" as const, statement: { type: "doc", content: [] } },
      evaluation: null,
    };
    invoke.mockResolvedValue(result);
    await expect(previewQuestionDraft(input)).resolves.toBe(result);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("preview_question_draft", { input });
  });

  it("preserves save identity and does not hydrate after a committed receipt", async () => {
    const input: SaveQuestionNoteInput = {
      operationId: "a".repeat(32),
      noteId: null,
      revision: null,
      deckId: "b".repeat(32),
      definitionJson: JSON.stringify({
        schemaVersion: 1,
        question: {
          kind: "select_all",
          prompt: { type: "doc", content: [] },
          options: [],
          correctOptionIds: [],
          explanation: null,
        },
      }),
      tags: [],
      allowDuplicate: false,
    };
    const receipt = {
      operationId: input.operationId,
      noteId: "c".repeat(32),
      noteRevision: 1,
      cardIds: ["d".repeat(32)],
      committedAt: 100,
    };
    invoke.mockResolvedValue(receipt);
    await expect(saveQuestionNote(input)).resolves.toEqual(receipt);
    await expect(saveQuestionNote(input)).resolves.toEqual(receipt);
    expect(invoke.mock.calls).toEqual([
      ["save_question_note", { input }],
      ["save_question_note", { input }],
    ]);
    expect(input.noteId).toBeNull();
    expect(input.operationId).toBe("a".repeat(32));
  });

  it("keeps reopening separate and propagates a read failure without another save", async () => {
    const failure = { code: "card_not_found", message: "That question no longer exists." };
    invoke.mockRejectedValue(failure);
    await expect(getQuestionNote("c".repeat(32))).rejects.toEqual(failure);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("get_question_note", {
      noteId: "c".repeat(32),
    });
  });
});

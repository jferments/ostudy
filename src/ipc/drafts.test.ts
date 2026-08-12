import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  commitEditorDraft,
  discardEditorDraft,
  listEditorDrafts,
  openEditorDraft,
  saveEditorDraft,
} from "./drafts";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const draftId = "d".repeat(32);
const operationId = "o".repeat(32);

beforeEach(() => vi.mocked(invoke).mockReset());

describe("durable draft IPC", () => {
  it("uses typed operation and optimistic-revision payloads", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    const payload = { frontText: "Question" };
    await openEditorDraft({
      operationId,
      draftKind: "card",
      contextEntityId: "c".repeat(32),
      payloadSchemaVersion: 1,
      initialPayload: payload,
    });
    expect(invoke).toHaveBeenLastCalledWith("open_editor_draft", {
      input: {
        operationId,
        draftKind: "card",
        contextEntityId: "c".repeat(32),
        payloadSchemaVersion: 1,
        initialPayload: payload,
      },
    });

    await saveEditorDraft({
      draftId,
      revision: 2,
      payloadSchemaVersion: 1,
      payload,
      assetIds: [],
    });
    expect(invoke).toHaveBeenLastCalledWith("save_editor_draft", {
      input: {
        draftId,
        revision: 2,
        payloadSchemaVersion: 1,
        payload,
        assetIds: [],
      },
    });
  });

  it("wires discard, commit, and recovery listing", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    await discardEditorDraft(draftId, 3);
    expect(invoke).toHaveBeenLastCalledWith("discard_editor_draft", {
      input: { draftId, revision: 3 },
    });
    await commitEditorDraft(draftId, 3, "e".repeat(32));
    expect(invoke).toHaveBeenLastCalledWith("commit_editor_draft", {
      input: { draftId, revision: 3, targetEntityId: "e".repeat(32) },
    });
    await listEditorDrafts();
    expect(invoke).toHaveBeenLastCalledWith("list_editor_drafts");
  });
});

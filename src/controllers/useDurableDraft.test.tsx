import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as draftIpc from "../ipc/drafts";
import * as platformIpc from "../ipc/platform";
import { discardActiveDurableDraft, useDurableDraft } from "./useDurableDraft";

vi.mock("../ipc/drafts", () => ({
  openEditorDraft: vi.fn(),
  saveEditorDraft: vi.fn(),
  discardEditorDraft: vi.fn(),
  commitEditorDraft: vi.fn(),
}));
vi.mock("../ipc/platform", () => ({ listenToLifecycle: vi.fn() }));

const id = "d".repeat(32);
const baseDraft = {
  id,
  draftKind: "bulk_cards" as const,
  targetEntityId: null,
  contextEntityId: "b".repeat(32),
  baseRevision: null,
  payloadSchemaVersion: 1,
  payload: { rows: [] },
  revision: 1,
  state: "active" as const,
  assetDraftId: null,
  createdAt: 1,
  updatedAt: 1,
  lastOpenedAt: 1,
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(platformIpc.listenToLifecycle).mockResolvedValue(() => undefined);
  vi.mocked(draftIpc.openEditorDraft).mockResolvedValue(baseDraft);
  vi.mocked(draftIpc.saveEditorDraft).mockImplementation(async (input) => ({
    ...baseDraft,
    payload: input.payload as { rows: unknown[] },
    revision: input.revision + 1,
    updatedAt: 2,
  }));
  vi.mocked(draftIpc.discardEditorDraft).mockResolvedValue(undefined);
  vi.mocked(draftIpc.commitEditorDraft).mockResolvedValue(undefined);
});

describe("useDurableDraft", () => {
  it("opens, flushes the newest payload, and commits by optimistic revision", async () => {
    const { result, rerender } = renderHook(
      ({ rows }) =>
        useDurableDraft({
          kind: "bulk_cards",
          contextEntityId: "b".repeat(32),
          payload: { rows },
        }),
      { initialProps: { rows: [] as string[] } },
    );
    await waitFor(() => expect(result.current.status).toBe("clean"));

    rerender({ rows: ["one", "two"] });
    await act(async () => result.current.flush());
    expect(draftIpc.saveEditorDraft).toHaveBeenCalledWith(
      expect.objectContaining({ payload: { rows: ["one", "two"] }, revision: 1 }),
    );

    await act(async () => result.current.commit());
    expect(draftIpc.commitEditorDraft).toHaveBeenCalledWith(id, 2, undefined);
  });

  it("applies recovered payloads and discards the durable record", async () => {
    const recovered = { ...baseDraft, payload: { rows: ["recovered"] }, updatedAt: 2 };
    vi.mocked(draftIpc.openEditorDraft).mockResolvedValueOnce(recovered);
    const onRecovered = vi.fn();
    const { result } = renderHook(() =>
      useDurableDraft({
        kind: "bulk_cards",
        contextEntityId: "b".repeat(32),
        payload: { rows: [] as string[] },
        onRecovered,
      }),
    );
    await waitFor(() => expect(onRecovered).toHaveBeenCalledWith({ rows: ["recovered"] }));
    await act(async () => result.current.discard());
    expect(draftIpc.discardEditorDraft).toHaveBeenCalledWith(id, 1);
  });

  it("lets confirmed app navigation discard the mounted card editor draft", async () => {
    const contextEntityId = "c".repeat(32);
    vi.mocked(draftIpc.openEditorDraft).mockResolvedValueOnce({
      ...baseDraft,
      draftKind: "card",
      contextEntityId,
      payload: { front: "question" },
    });
    const { result, unmount } = renderHook(() =>
      useDurableDraft({
        kind: "card",
        contextEntityId,
        payload: { front: "question" },
      }),
    );
    await waitFor(() => expect(result.current.status).toBe("clean"));

    await act(async () => discardActiveDurableDraft("card"));
    expect(draftIpc.discardEditorDraft).toHaveBeenCalledWith(id, 1);
    unmount();
  });

  it("treats Rust-reordered object keys as the same fresh payload", async () => {
    const contextEntityId = "b".repeat(32);
    const initialPayload = {
      selectedDeckId: contextEntityId,
      frontDoc: {
        type: "doc",
        content: [
          {
            type: "paragraph",
            attrs: { textAlign: null, indent: 0 },
          },
        ],
      },
      tags: ["android"],
    };
    const reorderedPayload = {
      frontDoc: {
        content: [
          {
            attrs: { indent: 0, textAlign: null },
            type: "paragraph",
          },
        ],
        type: "doc",
      },
      selectedDeckId: contextEntityId,
      tags: ["android"],
    };
    vi.mocked(draftIpc.openEditorDraft).mockResolvedValueOnce({
      ...baseDraft,
      draftKind: "card",
      contextEntityId,
      payload: reorderedPayload,
    });
    const onRecovered = vi.fn();
    const { result } = renderHook(() =>
      useDurableDraft({
        kind: "card",
        contextEntityId,
        payload: initialPayload,
        onRecovered,
      }),
    );

    await waitFor(() => expect(result.current.status).toBe("clean"));
    expect(result.current.recovered).toBe(false);
    expect(result.current.hasUnsavedLocalChanges).toBe(false);
    expect(onRecovered).not.toHaveBeenCalled();

    await act(async () => result.current.flush());
    expect(draftIpc.saveEditorDraft).not.toHaveBeenCalled();
  });
});

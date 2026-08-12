import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as draftIpc from "../ipc/drafts";
import * as platformIpc from "../ipc/platform";
import { useDurableDraft } from "./useDurableDraft";

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
});

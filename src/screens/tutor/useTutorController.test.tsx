import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getAiTaskDefaults, listAiProviderProfiles } from "../../ipc/aiProfiles";
import { getSemanticPreparation, listSemanticScopeChoices, prepareSemanticMaterial, previewSemanticPreparation } from "../../ipc/knowledge";
import { createTutorConversation, getTutorConversation, listTutorConversations, sendTutorTurn,
  type ConversationDetail, type ConversationSummary, type TutorTurn } from "../../ipc/tutor";
import { useTutorController } from "./useTutorController";

vi.mock("../../ipc/aiProfiles", () => ({ getAiTaskDefaults: vi.fn(), listAiProviderProfiles: vi.fn() }));
vi.mock("../../ipc/knowledge", () => ({ cancelLearningTask: vi.fn(), listSemanticScopeChoices: vi.fn(),
  getSemanticPreparation: vi.fn(), prepareSemanticMaterial: vi.fn(), previewSemanticPreparation: vi.fn(),
}));
vi.mock("../../ipc/tutor", () => ({ createTutorConversation: vi.fn(), deleteTutorConversation: vi.fn(),
  getTutorConversation: vi.fn(), listTutorConversations: vi.fn(), sendTutorTurn: vi.fn() }));

const conversation: ConversationSummary = {
  id: "a".repeat(32), title: "Geometry", scope: { kind: "general", id: null }, revision: 1, createdAt: 1, updatedAt: 1,
};
const turn: TutorTurn = {
  id: "b".repeat(32), operationId: "c".repeat(32), sequence: 1, mode: "explain", userText: "Explain triangles",
  assistantText: "A triangle has three sides.", state: "complete", citations: [], evidenceCurrent: true,
  pendingQuestion: false, feedback: null, provider: "llama_cpp", model: "fixture", errorCode: "", errorMessage: "", totalTokens: 20, createdAt: 1,
};
const detail: ConversationDetail = { conversation: { ...conversation, revision: 3 }, turns: [turn], hasEarlierTurns: false };

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(getAiTaskDefaults).mockResolvedValue({ flashcardGenerationProfileId: null, tutorProfileId: null, embeddingsProfileId: null });
  vi.mocked(listAiProviderProfiles).mockResolvedValue([]);
  vi.mocked(listSemanticScopeChoices).mockResolvedValue([]);
  vi.mocked(getSemanticPreparation).mockResolvedValue(null);
  vi.mocked(listTutorConversations).mockResolvedValue([]);
  vi.mocked(createTutorConversation).mockResolvedValue(conversation);
  vi.mocked(getTutorConversation).mockResolvedValue(detail);
  vi.mocked(sendTutorTurn).mockResolvedValue(turn);
});

describe("Tutor controller", () => {
  it("does not contact a provider or load the library merely by mounting", () => {
    renderHook(useTutorController);
    expect(listSemanticScopeChoices).not.toHaveBeenCalled();
    expect(sendTutorTurn).not.toHaveBeenCalled();
    expect(createTutorConversation).not.toHaveBeenCalled();
  });

  it("creates a conversation and sends through the existing typed native gateway", async () => {
    const { result } = renderHook(useTutorController);
    await act(async () => { await result.current.send("explain", "Explain triangles"); });
    expect(createTutorConversation).toHaveBeenCalledTimes(1);
    expect(sendTutorTurn).toHaveBeenCalledWith(expect.objectContaining({ conversationId: conversation.id,
      expectedRevision: 1, mode: "explain", text: "Explain triangles", focus: null, questionId: null }));
    expect(result.current.detail?.turns[0].assistantText).toBe(turn.assistantText);
  });

  it("retains the exact operation after an ambiguous result instead of duplicating a turn", async () => {
    vi.mocked(sendTutorTurn).mockRejectedValueOnce(new Error("Transport disconnected"));
    const { result } = renderHook(useTutorController);
    await act(async () => { await result.current.send("explain", "Explain triangles"); });
    const firstRequest = vi.mocked(sendTutorTurn).mock.calls[0][0];
    expect(result.current.pending).toEqual(firstRequest);
    await act(async () => { await result.current.retryPending(); });
    expect(vi.mocked(sendTutorTurn).mock.calls[1][0]).toEqual(firstRequest);
    expect(createTutorConversation).toHaveBeenCalledTimes(1);
    expect(result.current.pending).toBeNull();
  });

  it("does not resubmit a saved response when only the following refresh fails", async () => {
    vi.mocked(getTutorConversation).mockRejectedValueOnce(new Error("Refresh unavailable"));
    const { result } = renderHook(useTutorController);
    await act(async () => { await result.current.send("explain", "Explain triangles"); });
    expect(result.current.needsRefresh).toBe(true);
    expect(result.current.pending).toBeNull();
    expect(result.current.detail?.turns[0].id).toBe(turn.id);
    await act(async () => { await result.current.refresh(); });
    expect(sendTutorTurn).toHaveBeenCalledTimes(1);
    expect(result.current.needsRefresh).toBe(false);
  });

  it("prevents a double submission while native admission is pending", async () => {
    let resolve!: (value: TutorTurn) => void;
    vi.mocked(sendTutorTurn).mockReturnValueOnce(new Promise((done) => { resolve = done; }));
    const { result } = renderHook(useTutorController);
    let first!: Promise<void>;
    await act(async () => {
      first = result.current.send("explain", "First request");
      await result.current.send("explain", "Second request");
    });
    expect(sendTutorTurn).toHaveBeenCalledTimes(1);
    await act(async () => { resolve(turn); await first; });
  });

  it("starts a fresh hint context instead of displaying an earlier explanation", async () => {
    const { result } = renderHook(useTutorController);
    await act(async () => { await result.current.send("explain", "Explain triangles"); });
    await act(async () => {
      result.current.openContext({ title: "Current card", scope: { kind: "deck", id: "d".repeat(32) },
        focus: { kind: "card", sessionId: "e".repeat(32), cardId: "f".repeat(32), noteRevision: 1 }, mode: "hint" });
    });
    expect(result.current.detail).toBeNull();
    expect(result.current.target?.mode).toBe("hint");
    expect(result.current.panelOpen).toBe(true);
    expect(sendTutorTurn).toHaveBeenCalledTimes(1);
  });
});


describe("explicit conversation retrieval", () => {
  const deckScope = { kind: "deck" as const, id: "d".repeat(32) };

  it("defaults to exclusion and enables both preparation and retrieval only on request", async () => {
    vi.mocked(createTutorConversation).mockResolvedValue({ ...conversation, scope: deckScope });
    vi.mocked(getTutorConversation).mockResolvedValue({ ...detail, conversation: { ...detail.conversation, scope: deckScope } });
    const { result } = renderHook(useTutorController);
    await act(async () => { result.current.newConversation(deckScope); });
    expect(result.current.includeConversations).toBe(false);
    expect(result.current.preparation.selection.includeConversations).toBe(false);
    await act(async () => { result.current.setIncludeConversations(true); });
    expect(result.current.includeConversations).toBe(true);
    expect(result.current.preparation.selection).toEqual({ scope: deckScope, includeConversations: true });
    expect(prepareSemanticMaterial).not.toHaveBeenCalled();
    expect(previewSemanticPreparation).not.toHaveBeenCalled();
    expect(sendTutorTurn).not.toHaveBeenCalled();
    await act(async () => { await result.current.send("explain", "What did we discuss about triangles?"); });
    expect(sendTutorTurn).toHaveBeenCalledWith(expect.objectContaining({ includeConversations: true }));
  });

  it("resets opt-in when switching conversations or material scope", async () => {
    const { result } = renderHook(useTutorController);
    await act(async () => { result.current.newConversation(deckScope); });
    await act(async () => { result.current.setIncludeConversations(true); });
    await act(async () => { result.current.newConversation({ kind: "document", id: "e".repeat(32) }); });
    expect(result.current.includeConversations).toBe(false);
    await act(async () => { result.current.setIncludeConversations(true); });
    await act(async () => { await result.current.selectConversation(conversation.id); });
    expect(result.current.includeConversations).toBe(false);
    await act(async () => { result.current.setIncludeConversations(true); });
    expect(result.current.includeConversations).toBe(false); // General discussion cannot search the library.
  });

  it("cannot enable conversation retrieval in a hidden-answer context", async () => {
    vi.mocked(createTutorConversation).mockResolvedValue({ ...conversation, scope: deckScope });
    const { result } = renderHook(useTutorController);
    await act(async () => { result.current.newConversation(deckScope); });
    await act(async () => { result.current.setIncludeConversations(true); });
    await act(async () => {
      result.current.openContext({ scope: deckScope, title: "Current card", mode: "hint",
        focus: { kind: "card", sessionId: "e".repeat(32), cardId: "f".repeat(32), noteRevision: 1 } });
    });
    await act(async () => { result.current.setIncludeConversations(true); });
    expect(result.current.includeConversations).toBe(false);
    expect(result.current.preparation.selection.includeConversations).toBe(false);
    await act(async () => { await result.current.send("hint", "A small hint"); });
    expect(sendTutorTurn).toHaveBeenCalledWith(expect.objectContaining({ mode: "hint", includeConversations: false }));
  });

  it("does not change consent or request identity while an ambiguous turn needs reconciliation", async () => {
    vi.mocked(createTutorConversation).mockResolvedValue({ ...conversation, scope: deckScope });
    vi.mocked(sendTutorTurn).mockRejectedValueOnce(new Error("Disconnected"));
    const { result } = renderHook(useTutorController);
    await act(async () => { result.current.newConversation(deckScope); });
    await act(async () => { result.current.setIncludeConversations(true); });
    await act(async () => { await result.current.send("explain", "Explain the previous discussion"); });
    const original = vi.mocked(sendTutorTurn).mock.calls[0][0];
    await act(async () => { result.current.setIncludeConversations(false); });
    expect(result.current.includeConversations).toBe(true);
    expect(result.current.pending).toEqual(original);
    await act(async () => { await result.current.retryPending(); });
    expect(vi.mocked(sendTutorTurn).mock.calls[1][0]).toEqual(original);
  });
});

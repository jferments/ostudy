import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getEmbeddingOptions, listEmbeddingModels, saveEmbeddingOptions, testEmbeddingProfile } from "./embeddings";
import {
  cancelLearningTask,
  clearSemanticIndex,
  getLearningTaskProgress,
  getSemanticPreparation,
  getSemanticSourceCitation,
  listSemanticScopeChoices,
  prepareSemanticMaterial,
  previewSemanticPreparation,
  searchSemanticMaterial,
} from "./knowledge";
import {
  createTutorConversation,
  deleteTutorConversation,
  getTutorConversation,
  listTutorConversations,
  sendTutorTurn,
  type TutorTurnInput,
} from "./tutor";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(null);
});

describe("tutor IPC", () => {
  it("preserves the same operation and exact native focus on ambiguous retries", async () => {
    const input: TutorTurnInput = {
      operationId: "a".repeat(32),
      conversationId: "b".repeat(32),
      expectedRevision: 7,
      mode: "hint",
      text: "A small hint",
      focus: { kind: "card", sessionId: "c".repeat(32), cardId: "d".repeat(32), noteRevision: 3 },
      profileId: null,
      includeConversations: false,
      questionId: null,
    };
    await sendTutorTurn(input);
    await sendTutorTurn(input);
    expect(invokeMock.mock.calls).toEqual([
      ["send_tutor_turn", { input }],
      ["send_tutor_turn", { input }],
    ]);
    const payload = JSON.stringify(invokeMock.mock.calls);
    expect(payload).not.toContain("expectedAnswer");
    expect(payload).not.toContain("answerVisible");
    expect(payload).not.toContain("credential");
  });

  it("uses bounded native conversation reads and explicit deletion", async () => {
    const id = "a".repeat(32);
    await listTutorConversations();
    await createTutorConversation({ operationId: id, scope: { kind: "general", id: null }, title: "Geometry" });
    await getTutorConversation({ conversationId: id, beforeSequence: 50 });
    await deleteTutorConversation({ conversationId: id, expectedRevision: 3 });
    expect(invokeMock.mock.calls.map((call) => call[0])).toEqual([
      "list_tutor_conversations", "create_tutor_conversation", "get_tutor_conversation", "delete_tutor_conversation",
    ]);
    expect(invokeMock).toHaveBeenNthCalledWith(3, "get_tutor_conversation", {
      input: { conversationId: id, beforeSequence: 50 },
    });
  });

  it("separates preview, confirmed preparation, search, source resolution and cancellation", async () => {
    const id = "a".repeat(32);
    const selection = { scope: { kind: "deck" as const, id }, includeConversations: false };
    await listSemanticScopeChoices();
    await previewSemanticPreparation(selection);
    await prepareSemanticMaterial({ operationId: id, selection, consentKey: "b".repeat(64) });
    await getSemanticPreparation(selection);
    await getLearningTaskProgress(id);
    await cancelLearningTask(id);
    await searchSemanticMaterial({ operationId: id, selection, query: "A paraphrased idea" });
    await getSemanticSourceCitation({ sourceId: id, contentHash: "b".repeat(64) });
    await clearSemanticIndex();
    expect(invokeMock.mock.calls.map((call) => call[0])).toEqual([
      "list_semantic_scope_choices", "preview_semantic_preparation", "prepare_semantic_material",
      "get_semantic_preparation", "get_learning_task_progress", "cancel_learning_task",
      "search_semantic_material", "get_semantic_source_citation", "clear_semantic_index",
    ]);
    expect(invokeMock).toHaveBeenNthCalledWith(3, "prepare_semantic_material", {
      input: { operationId: id, selection, consentKey: "b".repeat(64) },
    });
  });

  it("keeps embedding options and fixed provider probes behind native commands", async () => {
    const id = "a".repeat(32);
    await getEmbeddingOptions(id);
    await saveEmbeddingOptions({ profileId: id, expectedRevision: 0, dimensions: null, inputPolicy: "automatic" });
    await listEmbeddingModels(id);
    await testEmbeddingProfile(id);
    expect(invokeMock.mock.calls.map((call) => call[0])).toEqual([
      "get_embedding_options", "save_embedding_options", "list_embedding_models", "test_embedding_profile",
    ]);
    expect(invokeMock).toHaveBeenLastCalledWith("test_embedding_profile", { input: { profileId: id } });
  });
});

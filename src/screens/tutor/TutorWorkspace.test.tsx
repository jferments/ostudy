import { act, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getAiTaskDefaults, listAiProviderProfiles } from "../../ipc/aiProfiles";
import { getSemanticPreparation, getSemanticSourceCitation, listSemanticScopeChoices,
  prepareSemanticMaterial, previewSemanticPreparation, type SourceCitation } from "../../ipc/knowledge";
import { getTutorConversation, listTutorConversations, type ConversationDetail } from "../../ipc/tutor";
import { TutorContext } from "./TutorContext";
import { TutorWorkspace } from "./TutorWorkspace";
import { useTutorController, type TutorController } from "./useTutorController";

vi.mock("../../ipc/aiProfiles", () => ({ getAiTaskDefaults: vi.fn(), listAiProviderProfiles: vi.fn() }));
vi.mock("../../ipc/knowledge", () => ({ cancelLearningTask: vi.fn(), listSemanticScopeChoices: vi.fn(),
  getSemanticPreparation: vi.fn(), getSemanticSourceCitation: vi.fn(), prepareSemanticMaterial: vi.fn(),
  previewSemanticPreparation: vi.fn(), getLearningTaskProgress: vi.fn() }));
vi.mock("../../ipc/tutor", () => ({ createTutorConversation: vi.fn(), deleteTutorConversation: vi.fn(),
  getTutorConversation: vi.fn(), listTutorConversations: vi.fn(), sendTutorTurn: vi.fn() }));
vi.mock("../../ui/overlays/overlayContext", () => ({ useConfirm: () => async () => true }));
vi.mock("./TutorMemoryControls", () => ({ TutorMemoryControls: () => null }));
vi.mock("../../ui/overlays/Dialog", () => ({
  Dialog: ({ title, children }: { title: string; children: ReactNode }) => <section role="dialog" aria-label={title}>{children}</section>,
}));

const scope = { kind: "deck" as const, id: "a".repeat(32) };
const citation: SourceCitation = {
  id: "b".repeat(32), kind: "document", ownerId: "c".repeat(32), title: "Triangle source",
  excerpt: "A triangle has three sides.", locator: null, origin: "source_document", contentHash: "d".repeat(64),
};
const detail: ConversationDetail = {
  conversation: { id: "e".repeat(32), title: "Geometry", scope, revision: 3, createdAt: 1, updatedAt: 1 },
  hasEarlierTurns: false,
  turns: [{ id: "f".repeat(32), operationId: "1".repeat(32), sequence: 1, mode: "explain",
    userText: "Explain triangles", assistantText: "Three sides", state: "complete", citations: [citation],
    evidenceCurrent: true, pendingQuestion: false, feedback: null, provider: "llama_cpp", model: "fixture",
    errorCode: "", errorMessage: "", totalTokens: null, createdAt: 1 }],
};
let controller: TutorController;
function Harness() {
  controller = useTutorController();
  return <TutorContext.Provider value={controller}><TutorWorkspace /></TutorContext.Provider>;
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(getAiTaskDefaults).mockResolvedValue({ flashcardGenerationProfileId: null, tutorProfileId: null, embeddingsProfileId: null });
  vi.mocked(listAiProviderProfiles).mockResolvedValue([]);
  vi.mocked(listSemanticScopeChoices).mockResolvedValue([{ scope, title: "Geometry" }]);
  vi.mocked(listTutorConversations).mockResolvedValue([]);
  vi.mocked(getSemanticPreparation).mockResolvedValue(null);
  vi.mocked(getTutorConversation).mockResolvedValue(detail);
});

describe("Tutor workspace consent and source reads", () => {
  it("exposes the opt-in without preparing or changing conversation permissions automatically", async () => {
    render(<Harness />);
    await act(async () => { controller.newConversation(scope); });
    fireEvent.click(screen.getByText("Material and model"));
    const checkbox = screen.getByRole("checkbox", { name: "Search permitted conversations in this material selection" }) as HTMLInputElement;
    expect(checkbox.checked).toBe(false);
    await act(async () => { fireEvent.click(checkbox); });
    expect(checkbox.checked).toBe(true);
    expect(controller.preparation.selection.includeConversations).toBe(true);
    expect(previewSemanticPreparation).not.toHaveBeenCalled();
    expect(prepareSemanticMaterial).not.toHaveBeenCalled();
  });

  it("does not expose conversation search in general discussion or flashcard hint mode", async () => {
    render(<Harness />);
    expect(screen.queryByRole("checkbox", { name: /Search permitted conversations/ })).toBeNull();
    await act(async () => {
      controller.openContext({ scope, title: "Current card", mode: "hint",
        focus: { kind: "card", cardId: "2".repeat(32), sessionId: "3".repeat(32), noteRevision: 1 } });
    });
    fireEvent.click(screen.getByText("Material and model"));
    expect(screen.queryByRole("checkbox", { name: /Search permitted conversations/ })).toBeNull();
    expect(controller.includeConversations).toBe(false);
  });

  it.each([false, true])("discards a late source read after changing conversations (rejected=%s)", async (rejected) => {
    let complete!: (value: SourceCitation) => void;
    let fail!: (reason: Error) => void;
    vi.mocked(getSemanticSourceCitation).mockReturnValueOnce(new Promise((resolve, reject) => {
      complete = resolve; fail = reject;
    }));
    render(<Harness />);
    await act(async () => { await controller.selectConversation(detail.conversation.id); });
    fireEvent.click(screen.getByRole("button", { name: "Source 1: Triangle source" }));
    await act(async () => { controller.newConversation(); });
    await act(async () => {
      if (rejected) fail(new Error("Obsolete source error"));
      else complete(citation);
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByText(citation.excerpt)).toBeNull();
    expect(screen.queryByText("Obsolete source error")).toBeNull();
  });
});

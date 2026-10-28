import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getAiTaskDefaults, listAiProviderProfiles } from "../../ipc/aiProfiles";
import { beginAssetDraft, cancelAssetDraft, importImage } from "../../ipc/assets";
import { getLearningTaskProgress, getSemanticPreparation, listSemanticScopeChoices } from "../../ipc/knowledge";
import {
  getTutorConversation,
  getTutorImageInput,
  listTutorConversations,
  listTutorProjects,
  sendTutorTurn,
  type ConversationDetail,
  type TutorTurn,
} from "../../ipc/tutor";
import { stageBrowserImage } from "../../platform/imageInput";
import { TutorContext } from "./TutorContext";
import { TutorWorkspace } from "./TutorWorkspace";
import { useTutorController, type TutorController } from "./useTutorController";

vi.mock("../../ipc/aiProfiles", () => ({ getAiTaskDefaults: vi.fn(), listAiProviderProfiles: vi.fn() }));
vi.mock("../../ipc/assets", () => ({ beginAssetDraft: vi.fn(), cancelAssetDraft: vi.fn(), importImage: vi.fn() }));
vi.mock("../../platform/imageInput", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../platform/imageInput")>(),
  stageBrowserImage: vi.fn(),
}));
vi.mock("../../platform/assets", () => ({ managedAssetUrl: (id: string) => `ostudy-asset://${id}` }));
vi.mock("../../ipc/knowledge", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../ipc/knowledge")>(),
  cancelLearningTask: vi.fn(),
  listSemanticScopeChoices: vi.fn(),
  getSemanticPreparation: vi.fn(),
  getLearningTaskProgress: vi.fn(),
}));
vi.mock("../../ipc/tutor", () => ({
  createTutorConversation: vi.fn(),
  draftTutorAnswerCards: vi.fn(),
  getTutorConversation: vi.fn(),
  getTutorImageInput: vi.fn(),
  listTutorConversations: vi.fn(),
  listTutorProjects: vi.fn(),
  sendTutorTurn: vi.fn(),
}));
vi.mock("../../ui/overlays/overlayContext", () => {
  const actions = { register: () => () => undefined, isTop: () => true, confirm: async () => true };
  return {
    useConfirm: () => actions.confirm,
    useOverlayParentId: () => null,
    useOverlayActions: () => actions,
    useOverlayContext: () => ({ topId: null, ...actions }),
  };
});
vi.mock("./TutorMemoryControls", () => ({ TutorMemoryControls: () => null }));
vi.mock("../../ui/overlays/Dialog", () => ({
  Dialog: ({ title, children }: { title: string; children: ReactNode }) => <section role="dialog" aria-label={title}>{children}</section>,
  Sheet: ({ title, children }: { title: string; children: ReactNode }) => <section role="dialog" aria-label={title}>{children}</section>,
}));

const scope = { kind: "general" as const, id: null };
const asked: TutorTurn = {
  id: "f".repeat(32), operationId: "1".repeat(32), sequence: 1, mode: "explain", userText: "What does this show?",
  assistantText: "A Krebs cycle diagram.", state: "complete", citations: [], evidenceCurrent: true, pendingQuestion: false,
  feedback: null, question: null, questionId: null, provider: "fixture", model: "fixture", errorCode: "", errorMessage: "",
  totalTokens: null, createdAt: 1, replacedByTurnId: null,
  attachments: [
    { kind: "image", assetId: "a".repeat(32), name: "cycle.png" },
    { kind: "text", name: "notes.md", text: "Citrate first." },
  ],
};
const detail: ConversationDetail = {
  conversation: {
    id: "e".repeat(32), title: "Biochemistry", scope, projectId: null, revision: 3, organizationRevision: 1,
    archivedAt: null, createdAt: 1, updatedAt: 1,
  },
  hasEarlierTurns: false,
  turns: [asked],
};

let controller: TutorController;
function Harness() {
  controller = useTutorController();
  return <TutorContext.Provider value={controller}><TutorWorkspace /></TutorContext.Provider>;
}

function choose(...files: File[]) {
  const picker = screen.getByLabelText("Choose pictures or files");
  fireEvent.change(picker, { target: { files } });
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(getAiTaskDefaults).mockResolvedValue({ flashcardGenerationProfileId: null, tutorProfileId: null, embeddingsProfileId: null });
  vi.mocked(listAiProviderProfiles).mockResolvedValue([]);
  vi.mocked(listSemanticScopeChoices).mockResolvedValue([]);
  vi.mocked(listTutorConversations).mockResolvedValue([]);
  vi.mocked(listTutorProjects).mockResolvedValue([]);
  vi.mocked(getSemanticPreparation).mockResolvedValue(null);
  vi.mocked(getLearningTaskProgress).mockResolvedValue(null);
  vi.mocked(getTutorConversation).mockResolvedValue(detail);
  vi.mocked(getTutorImageInput).mockResolvedValue(true);
  vi.mocked(cancelAssetDraft).mockResolvedValue(undefined);
  vi.mocked(beginAssetDraft).mockResolvedValue({ id: "d".repeat(32), purpose: "import", expiresAt: 1 });
  vi.mocked(stageBrowserImage).mockResolvedValue({ id: "s".repeat(32) } as never);
  vi.mocked(importImage).mockResolvedValue({
    id: "b".repeat(32), originalFilename: "lab.png", mimeType: "image/png", byteSize: 10, createdAt: 1,
  });
});

describe("Tutor attachments", () => {
  it("sends a picked picture and text file with the question, then clears them", async () => {
    vi.mocked(sendTutorTurn).mockResolvedValue({ ...asked, id: "9".repeat(32), sequence: 2 });
    render(<Harness />);
    await act(async () => { await controller.selectConversation(detail.conversation.id); });
    await act(async () => {
      choose(new File(["png"], "lab.png", { type: "image/png" }), new File(["Glycolysis first."], "steps.md", { type: "text/markdown" }));
    });
    const attached = await screen.findByRole("list", { name: "Attached" });
    await waitFor(() => expect(attached.textContent).not.toContain("adding"));
    expect(screen.getByRole("button", { name: "Remove lab.png" })).toBeTruthy();
    expect(stageBrowserImage).toHaveBeenCalled();
    expect(importImage).toHaveBeenCalledWith("s".repeat(32), "d".repeat(32));

    // A picture alone is a question.
    const send = screen.getByRole("button", { name: "Send" }) as HTMLButtonElement;
    await act(async () => { controller.setDraft(""); });
    expect(send.disabled).toBe(false);
    await act(async () => { fireEvent.click(send); });
    expect(sendTutorTurn).toHaveBeenCalledWith(expect.objectContaining({
      mode: "explain",
      text: "",
      attachments: [
        { kind: "image", assetId: "b".repeat(32), draftId: "d".repeat(32) },
        { kind: "text", name: "steps.md", text: "Glycolysis first." },
      ],
    }));
    expect(screen.queryByRole("list", { name: "Attached" })).toBeNull();
    expect(cancelAssetDraft).not.toHaveBeenCalled();
  });

  it("says before sending when the chat's model can't see pictures", async () => {
    vi.mocked(getTutorImageInput).mockResolvedValue(false);
    render(<Harness />);
    await act(async () => { await controller.selectConversation(detail.conversation.id); });
    await act(async () => { choose(new File(["png"], "lab.png", { type: "image/png" })); });
    expect(await screen.findByText(/model can't see pictures/u)).toBeTruthy();
    expect((screen.getByRole("button", { name: "Send" }) as HTMLButtonElement).disabled).toBe(true);
    // Removing the picture cancels its draft and lets the question go.
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Remove lab.png" })); });
    expect(cancelAssetDraft).toHaveBeenCalledWith("d".repeat(32));
    expect(screen.queryByText(/model can't see pictures/u)).toBeNull();
  });

  it("refuses a file it can't send and keeps the rest", async () => {
    render(<Harness />);
    await act(async () => { await controller.selectConversation(detail.conversation.id); });
    await act(async () => { choose(new File(["%PDF"], "chapter.pdf", { type: "application/pdf" })); });
    expect(screen.getByRole("alert").textContent).toContain("chapter.pdf can't be attached");
    expect(screen.queryByRole("list", { name: "Attached" })).toBeNull();
    expect(beginAssetDraft).not.toHaveBeenCalled();
  });

  it("shows what a question carried and sends it again with Regenerate", async () => {
    vi.mocked(sendTutorTurn).mockResolvedValue({ ...asked, id: "9".repeat(32), sequence: 2 });
    render(<Harness />);
    await act(async () => { await controller.selectConversation(detail.conversation.id); });
    expect(screen.getByRole("img", { name: "cycle.png" })).toBeTruthy();
    expect(screen.getByText("notes.md")).toBeTruthy();
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Regenerate" })); });
    expect(sendTutorTurn).toHaveBeenCalledWith(expect.objectContaining({
      replacesTurnId: asked.id,
      attachments: [
        { kind: "image", assetId: "a".repeat(32) },
        { kind: "text", name: "notes.md", text: "Citrate first." },
      ],
    }));
  });
});

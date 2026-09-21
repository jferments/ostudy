import { act, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getAiTaskDefaults, listAiProviderProfiles } from "../../ipc/aiProfiles";
import {
  getSemanticPreparation,
  getSemanticSourceCitation,
  listSemanticScopeChoices,
  prepareSemanticMaterial,
  previewSemanticPreparation,
  type SourceCitation,
} from "../../ipc/knowledge";
import {
  createTutorProject,
  getTutorConversation,
  listTutorConversations,
  listTutorProjects,
  moveTutorConversation,
  type ConversationDetail,
  type ConversationSummary,
  type TutorProject,
} from "../../ipc/tutor";
import { TutorContext } from "./TutorContext";
import { TutorWorkspace } from "./TutorWorkspace";
import { useTutorController, type TutorController } from "./useTutorController";

vi.mock("../../ipc/aiProfiles", () => ({
  getAiTaskDefaults: vi.fn(),
  listAiProviderProfiles: vi.fn(),
}));
vi.mock("../../ipc/knowledge", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../ipc/knowledge")>();
  return {
    ...actual,
    cancelLearningTask: vi.fn(),
    listSemanticScopeChoices: vi.fn(),
    getSemanticPreparation: vi.fn(),
    getSemanticSourceCitation: vi.fn(),
    prepareSemanticMaterial: vi.fn(),
    previewSemanticPreparation: vi.fn(),
    getLearningTaskProgress: vi.fn(),
  };
});
vi.mock("../../ipc/tutor", () => ({
  createTutorConversation: vi.fn(),
  createTutorProject: vi.fn(),
  createTutorProjectConversation: vi.fn(),
  deleteTutorConversation: vi.fn(),
  deleteTutorProject: vi.fn(),
  getTutorConversation: vi.fn(),
  listTutorConversations: vi.fn(),
  listTutorProjects: vi.fn(),
  moveTutorConversation: vi.fn(),
  renameTutorConversation: vi.fn(),
  sendTutorTurn: vi.fn(),
  setTutorConversationArchived: vi.fn(),
  setTutorProjectArchived: vi.fn(),
  updateTutorProject: vi.fn(),
}));
vi.mock("../../ui/overlays/overlayContext", () => ({ useConfirm: () => async () => true }));
vi.mock("./TutorMemoryControls", () => ({ TutorMemoryControls: () => null }));
vi.mock("../../ui/overlays/Dialog", () => ({
  Dialog: ({ title, children }: { title: string; children: ReactNode }) => (
    <section role="dialog" aria-label={title}>{children}</section>
  ),
  Sheet: ({ title, children }: { title: string; children: ReactNode }) => (
    <section role="dialog" aria-label={title}>{children}</section>
  ),
  DialogActions: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}));

const scope = { kind: "deck" as const, id: "a".repeat(32) };
const citation: SourceCitation = {
  id: "b".repeat(32),
  kind: "document",
  ownerId: "c".repeat(32),
  title: "Triangle source",
  excerpt: "A triangle has three sides.",
  locator: null,
  origin: "source_document",
  contentHash: "d".repeat(64),
};
const project: TutorProject = {
  id: "7".repeat(32),
  title: "Cardiology",
  description: "Hemodynamics",
  instructions: "Explain mechanisms first.",
  defaultScope: scope,
  revision: 1,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 1,
};
const projectConversation: ConversationSummary = {
  id: "8".repeat(32),
  title: "Project geometry",
  scope,
  projectId: project.id,
  revision: 1,
  organizationRevision: 1,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 2,
};

const detail: ConversationDetail = {
  conversation: {
    id: "e".repeat(32),
    title: "Geometry",
    scope,
    projectId: null,
    revision: 3,
    organizationRevision: 1,
    archivedAt: null,
    createdAt: 1,
    updatedAt: 1,
  },
  hasEarlierTurns: false,
  turns: [
    {
      id: "f".repeat(32),
      operationId: "1".repeat(32),
      sequence: 1,
      mode: "explain",
      userText: "Explain triangles",
      assistantText: "Three sides",
      state: "complete",
      citations: [citation],
      evidenceCurrent: true,
      pendingQuestion: false,
      feedback: null,
      provider: "llama_cpp",
      model: "fixture",
      errorCode: "",
      errorMessage: "",
      totalTokens: null,
      createdAt: 1,
    },
  ],
};
let controller: TutorController;
function Harness() {
  controller = useTutorController();
  return (
    <TutorContext.Provider value={controller}>
      <TutorWorkspace />
    </TutorContext.Provider>
  );
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(getAiTaskDefaults).mockResolvedValue({
    flashcardGenerationProfileId: null,
    tutorProfileId: null,
    embeddingsProfileId: null,
  });
  vi.mocked(listAiProviderProfiles).mockResolvedValue([]);
  vi.mocked(listSemanticScopeChoices).mockResolvedValue([{ scope, title: "Geometry" }]);
  vi.mocked(listTutorConversations).mockResolvedValue([]);
  vi.mocked(listTutorProjects).mockResolvedValue([]);
  vi.mocked(createTutorProject).mockResolvedValue(project);
  vi.mocked(moveTutorConversation).mockResolvedValue({ ...projectConversation, organizationRevision: 2 });
  vi.mocked(getSemanticPreparation).mockResolvedValue(null);
  vi.mocked(getTutorConversation).mockResolvedValue(detail);
});

describe("Tutor workspace consent and source reads", () => {
  it("keeps opt-in in chat settings without preparing or changing permissions automatically", async () => {
    render(<Harness />);
    await act(async () => { controller.newConversation(scope); });
    fireEvent.click(screen.getByRole("button", { name: "Chat settings" }));
    const checkbox = screen.getByRole("checkbox", {
      name: "Search permitted conversations in this material selection",
    });
    expect(checkbox.getAttribute("aria-checked")).toBe("false");
    await act(async () => { fireEvent.click(checkbox); });
    expect(checkbox.getAttribute("aria-checked")).toBe("true");
    expect(controller.preparation.selection.includeConversations).toBe(true);
    expect(previewSemanticPreparation).not.toHaveBeenCalled();
    expect(prepareSemanticMaterial).not.toHaveBeenCalled();
  });

  it("does not expose conversation search in general discussion or flashcard hint mode", async () => {
    render(<Harness />);
    expect(screen.queryByRole("checkbox", { name: /Search permitted conversations/ })).toBeNull();
    await act(async () => {
      controller.openContext({
        scope,
        title: "Current card",
        mode: "hint",
        focus: {
          kind: "card",
          cardId: "2".repeat(32),
          sessionId: "3".repeat(32),
          noteRevision: 1,
        },
      });
    });
    expect(screen.queryByRole("button", { name: "Chat settings" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Sources" })).toBeNull();
    expect(screen.queryByRole("checkbox", { name: /Search permitted conversations/ })).toBeNull();
    expect(controller.includeConversations).toBe(false);
  });

  it("keeps ordinary chat free of maintenance panels and restores saved history", async () => {
    render(<Harness />);
    await act(async () => { await controller.selectConversation(detail.conversation.id); });
    expect(screen.getByRole("textbox", { name: "Message" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sources" })).toBeTruthy();
    expect(screen.queryByText("Search index maintenance")).toBeNull();
    expect(screen.queryByText("Restore message to composer")).toBeNull();
    expect(screen.queryByText("Prepare selected material")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Conversations" }));
    expect(screen.getByRole("navigation", { name: "Conversations" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Geometry" })).toBeTruthy();
    expect(prepareSemanticMaterial).not.toHaveBeenCalled();
  });

  it("applies a source checklist to a new chat without editing prior conversation scope", async () => {
    const second = { kind: "document" as const, id: "4".repeat(32) };
    vi.mocked(listSemanticScopeChoices).mockResolvedValue([
      { scope, title: "Geometry deck" }, { scope: second, title: "Physics document" },
    ]);
    render(<Harness />);
    await act(async () => { await controller.selectConversation(detail.conversation.id); });
    await act(async () => { controller.setDraft("Compare the ideas"); });
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Sources" })); });
    fireEvent.click(screen.getByRole("checkbox", { name: /Physics document/ }));
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Use sources" })); });
    expect(controller.scope.kind).toBe("selection");
    expect(controller.detail).toBeNull();
    expect(controller.draft).toBe("Compare the ideas");
    expect(detail.conversation.scope).toEqual(scope);
    expect(previewSemanticPreparation).not.toHaveBeenCalled();
    expect(prepareSemanticMaterial).not.toHaveBeenCalled();
  });

  it("groups project chats and exposes project actions through the shared overflow menu", async () => {
    vi.mocked(listTutorProjects).mockResolvedValue([project]);
    vi.mocked(listTutorConversations).mockResolvedValue([projectConversation]);
    render(<Harness />);
    await act(async () => { await controller.initialize(); });
    fireEvent.click(screen.getByRole("button", { name: "Conversations" }));
    expect(screen.getByRole("button", { name: "Project geometry" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cardiology project actions" }));
    expect(await screen.findByRole("menuitem", { name: "Edit project" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Archive project" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Delete project" })).toBeTruthy();
  });

  it("moves a project conversation without applying destination defaults unless explicitly selected", async () => {
    vi.mocked(listTutorProjects).mockResolvedValue([project]);
    vi.mocked(listTutorConversations).mockResolvedValue([projectConversation]);
    vi.mocked(getTutorConversation).mockResolvedValue({ ...detail, conversation: projectConversation });
    render(<Harness />);
    await act(async () => { await controller.initialize(); });
    fireEvent.click(screen.getByRole("button", { name: "Conversations" }));
    fireEvent.click(screen.getByRole("button", { name: "Project geometry actions" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Move" }));
    const destination = screen.getByRole("combobox", { name: "Destination project" });
    fireEvent.change(destination, { target: { value: "" } });
    expect(screen.getByRole("checkbox", { name: "Use destination project's current default sources" }).getAttribute("aria-checked")).toBe("false");
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Move conversation" })); });
    expect(moveTutorConversation).toHaveBeenCalledWith(expect.objectContaining({
      conversationId: projectConversation.id,
      projectId: null,
      applyProjectDefault: false,
    }));
  });

  it("creates a Tutor project locally without preparing material or sending a Tutor turn", async () => {
    render(<Harness />);
    await act(async () => { await controller.initialize(); });
    fireEvent.click(screen.getByRole("button", { name: "Conversations" }));
    fireEvent.click(screen.getByRole("button", { name: "New project" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Project name" }), { target: { value: "Cardiology" } });
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Create project" })); });
    expect(createTutorProject).toHaveBeenCalledWith(expect.objectContaining({
      title: "Cardiology",
      defaultScope: { kind: "general", id: null },
    }));
    expect(prepareSemanticMaterial).not.toHaveBeenCalled();
  });

  it.each([false, true])(
    "discards a late source read after changing conversations (rejected=%s)",
    async (rejected) => {
      let complete!: (value: SourceCitation) => void;
      let fail!: (reason: Error) => void;
      vi.mocked(getSemanticSourceCitation).mockReturnValueOnce(
        new Promise((resolve, reject) => {
          complete = resolve;
          fail = reject;
        }),
      );
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
    },
  );
});

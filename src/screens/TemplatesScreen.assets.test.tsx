import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../ipc";
import { TemplatesScreen } from "./TemplatesScreen";

vi.mock("../ipc", () => ({
  beginAssetDraft: vi.fn(),
  cancelAssetDraft: vi.fn(),
  createCardTemplate: vi.fn(),
  deleteCardTemplate: vi.fn(),
  getCardTemplate: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  listCardTemplateSummaries: vi.fn(),
  syncAssetDraft: vi.fn(),
  updateCardTemplate: vi.fn(),
}));

vi.mock("./templates/TemplateLayoutEditor", () => ({
  TemplateLayoutEditor: ({ label }: { label: string }) => <div>{label}</div>,
}));

const draftId = "a".repeat(32);

function savedTemplate(): ipc.CardTemplate {
  return {
    id: "b".repeat(32),
    name: "Medication",
    description: "",
    fields: [
      {
        id: "c".repeat(32),
        name: "Field",
        description: "",
        position: 0,
      },
    ],
    rules: [
      {
        id: "d".repeat(32),
        name: "Card",
        frontDoc: { type: "doc", content: [] },
        backDoc: { type: "doc", content: [] },
        position: 0,
      },
    ],
    assets: [],
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  };
}

describe("TemplatesScreen managed assets", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.listCardTemplateSummaries).mockResolvedValue([]);
    vi.mocked(ipc.beginAssetDraft).mockResolvedValue({
      id: draftId,
      purpose: "template",
      expiresAt: Date.now() + 10_000,
    });
    vi.mocked(ipc.cancelAssetDraft).mockResolvedValue(undefined);
    vi.mocked(ipc.syncAssetDraft).mockResolvedValue(undefined);
    vi.mocked(ipc.createCardTemplate).mockResolvedValue(savedTemplate());
  });

  it("opens one template draft and commits its identity with the template", async () => {
    render(<TemplatesScreen onDirtyChange={vi.fn()} showToast={vi.fn()} />);

    await screen.findByRole("heading", { name: "Create a reusable card template" });
    fireEvent.click(screen.getByRole("button", { name: "New template" }));

    await waitFor(() => {
      expect(ipc.beginAssetDraft).toHaveBeenCalledWith("template", undefined);
    });

    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "Medication" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save template" }));

    await waitFor(() => {
      expect(ipc.createCardTemplate).toHaveBeenCalledWith(
        expect.objectContaining({
          name: "Medication",
          assetDraftId: draftId,
        }),
      );
    });
  });

  it("cancels the uncommitted draft when the editor is canceled", async () => {
    render(<TemplatesScreen onDirtyChange={vi.fn()} showToast={vi.fn()} />);

    await screen.findByRole("heading", { name: "Create a reusable card template" });
    fireEvent.click(screen.getByRole("button", { name: "New template" }));
    await waitFor(() => expect(ipc.beginAssetDraft).toHaveBeenCalledOnce());

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(ipc.cancelAssetDraft).toHaveBeenCalledWith(draftId));
  });
});

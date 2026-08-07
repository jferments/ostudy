import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as assetsIpc from "../ipc/assets";
import * as templatesIpc from "../ipc/templates";
import { deferred } from "../test/factories";
import { TemplatesScreen } from "./TemplatesScreen";

vi.mock("../ipc/assets", () => ({
  beginAssetDraft: vi.fn(),
  cancelAssetDraft: vi.fn(),
  syncAssetDraft: vi.fn(),
}));
vi.mock("../ipc/common", () => ({
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));
vi.mock("../ipc/templates", () => ({
  createCardTemplate: vi.fn(),
  deleteCardTemplate: vi.fn(),
  getCardTemplate: vi.fn(),
  listCardTemplateSummaries: vi.fn(),
  updateCardTemplate: vi.fn(),
}));

vi.mock("./templates/TemplateLayoutEditor", () => ({
  TemplateLayoutEditor: ({ label }: { label: string }) => <div>{label}</div>,
}));

const draftId = "a".repeat(32);

function savedTemplate(): templatesIpc.CardTemplate {
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
    vi.mocked(templatesIpc.listCardTemplateSummaries).mockResolvedValue([]);
    vi.mocked(assetsIpc.beginAssetDraft).mockResolvedValue({
      id: draftId,
      purpose: "template",
      expiresAt: Date.now() + 10_000,
    });
    vi.mocked(assetsIpc.cancelAssetDraft).mockResolvedValue(undefined);
    vi.mocked(assetsIpc.syncAssetDraft).mockResolvedValue(undefined);
    vi.mocked(templatesIpc.createCardTemplate).mockResolvedValue(savedTemplate());
  });

  it("opens one template draft and commits its identity with the template", async () => {
    render(<TemplatesScreen onDirtyChange={vi.fn()} showToast={vi.fn()} />);

    await screen.findByRole("heading", { name: "Create a reusable card template" });
    fireEvent.click(screen.getByRole("button", { name: "New template" }));

    await waitFor(() => {
      expect(assetsIpc.beginAssetDraft).toHaveBeenCalledWith("template", undefined);
    });

    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "Medication" },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Save template" }));

    await waitFor(() => {
      expect(templatesIpc.createCardTemplate).toHaveBeenCalledWith(
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
    await waitFor(() => expect(assetsIpc.beginAssetDraft).toHaveBeenCalledOnce());

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(assetsIpc.cancelAssetDraft).toHaveBeenCalledWith(draftId));
  });

  it("blocks save and cancels a managed draft that resolves after cancellation", async () => {
    const opening = deferred<Awaited<ReturnType<typeof assetsIpc.beginAssetDraft>>>();
    vi.mocked(assetsIpc.beginAssetDraft).mockReturnValueOnce(opening.promise);
    vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<TemplatesScreen onDirtyChange={vi.fn()} showToast={vi.fn()} />);

    await screen.findByRole("heading", { name: "Create a reusable card template" });
    fireEvent.click(screen.getByRole("button", { name: "New template" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "Medication" },
    });

    const preparing = screen.getByRole("button", { name: "Preparing…" }) as HTMLButtonElement;
    expect(preparing.disabled).toBe(true);
    fireEvent.click(preparing);
    expect(templatesIpc.createCardTemplate).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    opening.resolve({
      id: draftId,
      purpose: "template",
      expiresAt: Date.now() + 10_000,
    });

    await waitFor(() => expect(assetsIpc.cancelAssetDraft).toHaveBeenCalledWith(draftId));
    expect(templatesIpc.createCardTemplate).not.toHaveBeenCalled();
    expect(screen.getByRole("heading", { name: "Create a reusable card template" })).toBeTruthy();
  });
});

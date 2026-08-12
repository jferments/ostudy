import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as templatesIpc from "../ipc/templates";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { TemplatesScreen } from "./TemplatesScreen";

const durableDraftMock = vi.hoisted(() => ({
  draftId: "d".repeat(32),
  assetDraftId: "7".repeat(32) as string | null,
  revision: 1,
  status: "saved" as
    | "opening"
    | "clean"
    | "locally_dirty"
    | "saving"
    | "saved"
    | "save_failed"
    | "conflicted"
    | "committing",
  errorMessage: null as string | null,
  recovered: false,
  hasUnsavedLocalChanges: false,
  flush: vi.fn().mockResolvedValue(undefined),
  discard: vi.fn().mockResolvedValue(undefined),
  commit: vi.fn().mockResolvedValue(undefined),
  restart: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../controllers/useDurableDraft", () => ({
  useDurableDraft: () => durableDraftMock,
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

const assetDraftId = "7".repeat(32);

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

function renderTemplates() {
  return render(
    <OverlayProvider>
      <TemplatesScreen onDirtyChange={vi.fn()} showToast={vi.fn()} />
    </OverlayProvider>,
  );
}

describe("TemplatesScreen durable managed assets", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    durableDraftMock.status = "saved";
    durableDraftMock.assetDraftId = assetDraftId;
    durableDraftMock.errorMessage = null;
    durableDraftMock.hasUnsavedLocalChanges = false;
    durableDraftMock.flush.mockResolvedValue(undefined);
    durableDraftMock.discard.mockResolvedValue(undefined);
    durableDraftMock.commit.mockResolvedValue(undefined);
    vi.mocked(templatesIpc.listCardTemplateSummaries).mockResolvedValue([]);
    vi.mocked(templatesIpc.createCardTemplate).mockResolvedValue(savedTemplate());
  });

  it("uses the persistent draft asset identity when saving a template", async () => {
    renderTemplates();

    await screen.findByRole("heading", { name: "Create a reusable card template" });
    fireEvent.click(screen.getByRole("button", { name: "New template" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "Medication" },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Save template" }));

    await waitFor(() => {
      expect(templatesIpc.createCardTemplate).toHaveBeenCalledWith(
        expect.objectContaining({ name: "Medication", assetDraftId }),
      );
      expect(durableDraftMock.flush).toHaveBeenCalled();
      expect(durableDraftMock.commit).toHaveBeenCalledWith(savedTemplate().id);
    });
  });

  it("discards the persistent draft when edited template work is canceled", async () => {
    durableDraftMock.hasUnsavedLocalChanges = true;
    renderTemplates();

    await screen.findByRole("heading", { name: "Create a reusable card template" });
    fireEvent.click(screen.getByRole("button", { name: "New template" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "Medication" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Discard changes" }),
    );

    await waitFor(() => expect(durableDraftMock.discard).toHaveBeenCalledOnce());
    expect(
      await screen.findByRole("heading", { name: "Create a reusable card template" }),
    ).toBeTruthy();
  });

  it("blocks save while the durable draft is opening and still permits discard", async () => {
    durableDraftMock.status = "opening";
    durableDraftMock.assetDraftId = null;
    durableDraftMock.hasUnsavedLocalChanges = true;
    renderTemplates();

    await screen.findByRole("heading", { name: "Create a reusable card template" });
    fireEvent.click(screen.getByRole("button", { name: "New template" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "Medication" },
    });

    const preparing = await screen.findByRole("button", { name: "Preparing…" });
    expect((preparing as HTMLButtonElement).disabled).toBe(true);
    expect(templatesIpc.createCardTemplate).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Discard changes" }),
    );

    await waitFor(() => expect(durableDraftMock.discard).toHaveBeenCalledOnce());
    expect(templatesIpc.createCardTemplate).not.toHaveBeenCalled();
  });
});

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as assetsIpc from "../ipc/assets";
import * as commonIpc from "../ipc/common";
import * as templatesIpc from "../ipc/templates";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { TemplatesScreen } from "./TemplatesScreen";

vi.mock("../controllers/useDurableDraft", () => ({
  useDurableDraft: () => ({
    draftId: "d".repeat(32),
    assetDraftId: "7".repeat(32),
    revision: 1,
    status: "saved",
    errorMessage: null,
    recovered: false,
    hasUnsavedLocalChanges: false,
    flush: vi.fn().mockResolvedValue(undefined),
    discard: vi.fn().mockResolvedValue(undefined),
    prepareCanonicalCommit: vi.fn().mockResolvedValue({ draftId: "d".repeat(32), revision: 1 }),
    acceptCanonicalCommit: vi.fn(),
    commit: vi.fn().mockResolvedValue(undefined),
    restart: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock("../ipc/assets", () => ({
  beginAssetDraft: vi.fn(),
  cancelAssetDraft: vi.fn(),
  syncAssetDraft: vi.fn(),
}));
vi.mock("../ipc/common", () => ({
  getErrorMessage: vi.fn((error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  ),
}));
vi.mock("../ipc/templates", () => ({
  createCardTemplate: vi.fn(),
  deleteCardTemplate: vi.fn(),
  getCardTemplate: vi.fn(),
  listCardTemplateSummaries: vi.fn(),
  updateCardTemplate: vi.fn(),
}));

const FIELD_ID = "b".repeat(32);
const ASSET_DRAFT_ID = "7".repeat(32);
const inlineDoc: commonIpc.RichDocument = {
  type: "doc",
  content: [
    {
      type: "paragraph",
      content: [
        { type: "text", text: "Question about " },
        {
          type: "templateFieldInline",
          attrs: { fieldId: FIELD_ID, label: "Concept" },
        },
      ],
    },
  ],
};

const template: templatesIpc.CardTemplate = {
  id: "a".repeat(32),
  name: "Concept review",
  description: "Reusable concept cards",
  fields: [
    {
      id: FIELD_ID,
      name: "Concept",
      description: "Enter a description of this field.",
      position: 0,
    },
  ],
  rules: [
    {
      id: "d".repeat(32),
      name: "Concept card",
      frontDoc: inlineDoc,
      backDoc: {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [
              {
                type: "templateFieldInline",
                attrs: { fieldId: FIELD_ID, label: "Concept" },
              },
            ],
          },
        ],
      },
      position: 0,
    },
  ],
  assets: [],
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

const summary: templatesIpc.CardTemplateSummary = {
  id: template.id,
  name: template.name,
  description: template.description,
  fieldCount: 1,
  generatedCardRuleCount: 1,
  imageCount: 0,
  updatedAt: 1,
  revision: 1,
};

function renderTemplates(showToast = vi.fn()) {
  render(
    <OverlayProvider>
      <TemplatesScreen onDirtyChange={vi.fn()} showToast={showToast} />
    </OverlayProvider>,
  );
  return showToast;
}

describe("TemplatesScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(assetsIpc.beginAssetDraft).mockResolvedValue({
      id: ASSET_DRAFT_ID,
      purpose: "template",
      expiresAt: Date.now() + 60_000,
    });
    vi.mocked(assetsIpc.cancelAssetDraft).mockResolvedValue(undefined);
    vi.mocked(assetsIpc.syncAssetDraft).mockResolvedValue(undefined);
    vi.mocked(templatesIpc.listCardTemplateSummaries).mockResolvedValue([summary]);
    vi.mocked(templatesIpc.getCardTemplate).mockResolvedValue(template);
    vi.mocked(templatesIpc.createCardTemplate).mockResolvedValue(template);
    vi.mocked(templatesIpc.deleteCardTemplate).mockResolvedValue(undefined);
  });

  it("creates a template with typed placeholders and automatic placement", async () => {
    const showToast = renderTemplates();

    fireEvent.click(await screen.findByRole("button", { name: "New template" }));
    fireEvent.change(screen.getByLabelText("Name"), {
      target: { value: "Concept review" },
    });

    expect(screen.getByText("Field description")).toBeTruthy();
    expect(screen.getAllByText("Question layout").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Answer layout").length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Type a field name in double braces/u).length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: "Insert inline" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Insert as block" })).toBeNull();
    expect(screen.queryByText("Insert field")).toBeNull();

    fireEvent.click(await screen.findByRole("button", { name: "Save template" }));

    await waitFor(() => {
      expect(templatesIpc.createCardTemplate).toHaveBeenCalledWith(
        expect.objectContaining({
          name: "Concept review",
          assetDraftId: ASSET_DRAFT_ID,
          fields: [expect.objectContaining({ name: "Field" })],
          rules: [
            expect.objectContaining({
              frontDoc: expect.objectContaining({ type: "doc" }),
              backDoc: expect.objectContaining({ type: "doc" }),
            }),
          ],
        }),
      );
      expect(showToast).toHaveBeenCalledWith("success", "Card template created.");
    });
    expect(templatesIpc.listCardTemplateSummaries).toHaveBeenCalledTimes(2);
  });

  it("loads one full template graph only when editing", async () => {
    renderTemplates();

    expect(await screen.findByText("Reusable concept cards")).toBeTruthy();
    expect(templatesIpc.getCardTemplate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));

    expect(await screen.findByRole("heading", { name: "Edit card template" })).toBeTruthy();
    expect(templatesIpc.getCardTemplate).toHaveBeenCalledWith(template.id);
  });

  it("deletes only the authoring template after shared confirmation", async () => {
    const showToast = renderTemplates();

    expect(await screen.findByText("Reusable concept cards")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(
      await screen.findByRole("alertdialog", { name: "Delete “Concept review”?" }),
    ).toBeTruthy();
    expect(templatesIpc.deleteCardTemplate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Delete template" }));

    await waitFor(() => {
      expect(templatesIpc.deleteCardTemplate).toHaveBeenCalledWith(template.id);
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Deleted “Concept review”. Existing cards were not changed.",
      );
    });
  });
});

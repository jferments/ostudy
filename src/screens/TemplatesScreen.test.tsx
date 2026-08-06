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
  getErrorMessage: vi.fn((error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  ),
  listCardTemplateSummaries: vi.fn(),
  syncAssetDraft: vi.fn(),
  updateCardTemplate: vi.fn(),
}));

const FIELD_ID = "b".repeat(32);
const inlineDoc: ipc.RichDocument = {
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

const template: ipc.CardTemplate = {
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

const summary: ipc.CardTemplateSummary = {
  id: template.id,
  name: template.name,
  description: template.description,
  fieldCount: 1,
  generatedCardRuleCount: 1,
  imageCount: 0,
  updatedAt: 1,
  revision: 1,
};

describe("TemplatesScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.beginAssetDraft).mockResolvedValue({
      id: "e".repeat(32),
      purpose: "template",
      expiresAt: Date.now() + 60_000,
    });
    vi.mocked(ipc.cancelAssetDraft).mockResolvedValue(undefined);
    vi.mocked(ipc.syncAssetDraft).mockResolvedValue(undefined);
    vi.mocked(ipc.listCardTemplateSummaries).mockResolvedValue([summary]);
    vi.mocked(ipc.getCardTemplate).mockResolvedValue(template);
    vi.mocked(ipc.createCardTemplate).mockResolvedValue(template);
    vi.mocked(ipc.deleteCardTemplate).mockResolvedValue(undefined);
  });

  it("creates a template with typed placeholders and automatic placement", async () => {
    const showToast = vi.fn();
    render(<TemplatesScreen onDirtyChange={vi.fn()} showToast={showToast} />);

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

    fireEvent.click(screen.getByRole("button", { name: "Save template" }));

    await waitFor(() => {
      expect(ipc.createCardTemplate).toHaveBeenCalledWith(
        expect.objectContaining({
          name: "Concept review",
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
    expect(ipc.listCardTemplateSummaries).toHaveBeenCalledTimes(2);
  });

  it("loads one full template graph only when editing", async () => {
    render(<TemplatesScreen onDirtyChange={vi.fn()} showToast={vi.fn()} />);

    expect(await screen.findByText("Reusable concept cards")).toBeTruthy();
    expect(ipc.getCardTemplate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));

    expect(await screen.findByRole("heading", { name: "Edit card template" })).toBeTruthy();
    expect(ipc.getCardTemplate).toHaveBeenCalledWith(template.id);
    expect(ipc.beginAssetDraft).toHaveBeenCalledWith("template", template.id);
  });

  it("deletes only the authoring template and leaves generated cards intact", async () => {
    const showToast = vi.fn();
    vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<TemplatesScreen onDirtyChange={vi.fn()} showToast={showToast} />);

    expect(await screen.findByText("Reusable concept cards")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    await waitFor(() => {
      expect(ipc.deleteCardTemplate).toHaveBeenCalledWith(template.id);
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Deleted “Concept review”. Existing cards were not changed.",
      );
    });
  });
});

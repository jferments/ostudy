import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../ipc";
import { TemplatesScreen } from "./TemplatesScreen";

vi.mock("../ipc", () => ({
  createCardTemplate: vi.fn(),
  deleteCardTemplate: vi.fn(),
  getErrorMessage: vi.fn((error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  ),
  listCardTemplates: vi.fn(),
  updateCardTemplate: vi.fn(),
}));

const template: ipc.CardTemplate = {
  id: "a".repeat(32),
  name: "Pharmacology",
  description: "Medication facts",
  fields: [
    {
      id: "b".repeat(32),
      name: "Name",
      description: "Medication name",
      position: 0,
    },
    {
      id: "c".repeat(32),
      name: "Drug class",
      description: "Therapeutic class",
      position: 1,
    },
  ],
  rules: [
    {
      id: "d".repeat(32),
      name: "Class",
      frontPattern: "What is the drug class of {{Name}}?",
      backPattern: "{{Drug class}}",
      position: 0,
    },
  ],
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

describe("TemplatesScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.listCardTemplates).mockResolvedValue([template]);
    vi.mocked(ipc.createCardTemplate).mockResolvedValue(template);
    vi.mocked(ipc.deleteCardTemplate).mockResolvedValue();
  });

  it("creates a data-only template with fields and card rules", async () => {
    const showToast = vi.fn();
    render(
      <TemplatesScreen onDirtyChange={vi.fn()} showToast={showToast} />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "New template" }));
    fireEvent.change(screen.getByLabelText("Name"), {
      target: { value: "Pharmacology" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save template" }));

    await waitFor(() => {
      expect(ipc.createCardTemplate).toHaveBeenCalledWith(
        expect.objectContaining({
          name: "Pharmacology",
          fields: [
            expect.objectContaining({ name: "Name" }),
          ],
          rules: [
            expect.objectContaining({
              frontPattern: "What is {{Name}}?",
              backPattern: "{{Name}}",
            }),
          ],
        }),
      );
      expect(showToast).toHaveBeenCalledWith("success", "Card template created.");
    });
  });

  it("deletes only the authoring template and explains that generated cards remain", async () => {
    const showToast = vi.fn();
    vi.spyOn(window, "confirm").mockReturnValue(true);
    render(
      <TemplatesScreen onDirtyChange={vi.fn()} showToast={showToast} />,
    );

    expect(await screen.findByText("Medication facts")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    await waitFor(() => {
      expect(ipc.deleteCardTemplate).toHaveBeenCalledWith(template.id);
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Deleted “Pharmacology”. Existing cards were not changed.",
      );
    });
  });
});

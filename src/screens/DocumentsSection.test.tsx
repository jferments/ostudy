import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DocumentsSection } from "./DocumentsSection";

vi.mock("./DocumentsScreen", () => ({
  DocumentsScreen: () => (
    <div>
      <h1>Documents</h1>
      <div>Document library content</div>
    </div>
  ),
}));

describe("DocumentsSection", () => {
  it("shows the documents without a model or an AI setup notice", async () => {
    render(
      <DocumentsSection
        refreshDecks={vi.fn()}
        onPracticeCards={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    expect(await screen.findByText("Document library content")).toBeTruthy();
    expect(screen.queryByText(/model/iu)).toBeNull();
    expect(screen.queryByText(/Set up an AI provider/u)).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
  });
});

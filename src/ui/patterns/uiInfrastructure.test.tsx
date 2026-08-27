import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { artworkManifest } from "../artwork/artworkManifest";
import { IconButton } from "../icons/IconButton";
import { FilterSummary } from "./WorkspacePatterns";

describe("UI infrastructure", () => {
  it("requires an accessible name for icon-only actions", () => {
    render(<IconButton icon="search" label="Search flashcards" />);
    const button = screen.getByRole("button", { name: "Search flashcards" });
    expect(button.hasAttribute("title")).toBe(false);
    fireEvent.focus(button);
    expect(screen.getByRole("tooltip").textContent).toBe("Search flashcards");
  });

  it("registers semantic artwork slots independently from asset filenames", () => {
    expect(artworkManifest["empty.deck"].decorative).toBe(true);
    expect(artworkManifest["brand.logo.compact"].decorative).toBe(false);
  });

  it("offers direct removal and clearing of active filters", () => {
    render(
      <FilterSummary
        filters={[{ id: "due", label: "Ready now" }]}
        onRemove={() => undefined}
        onClear={() => undefined}
      />,
    );
    expect(screen.getByRole("button", { name: "Remove Ready now filter" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Clear all" })).toBeTruthy();
  });
});

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { IconButton } from "../icons/IconButton";
import { artworkManifest } from "../artwork/artworkManifest";
import { FilterSummary } from "./WorkspacePatterns";

describe("UI infrastructure", () => {
  it("requires an accessible name for icon-only actions", () => {
    render(<IconButton icon="search" label="Search flashcards" />);
    expect(screen.getByRole("button", { name: "Search flashcards" })).toBeInTheDocument();
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
    expect(screen.getByRole("button", { name: "Remove Ready now filter" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Clear all" })).toBeInTheDocument();
  });
});

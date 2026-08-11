import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { DeckSummary } from "../../ipc/decks";
import { CardFilters } from "./CardFilters";

const deck: DeckSummary = {
  id: "a".repeat(32),
  name: "Pharmacology",
  description: "",
  sourceNoteCount: 2,
  generatedCardCount: 2,
  studyableDueCount: 1,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

describe("CardFilters", () => {
  it("does not offer alphabetical question sorting", () => {
    render(
      <CardFilters
        activeDecks={[deck]}
        deckIds={new Set()}
        query=""
        availabilityFilter="all"
        statusFilter="active"
        reviewedFilter="all"
        sort="updated_desc"
        tags={[]}
        tagIds={new Set()}
        onDeckIdsChange={vi.fn()}
        onQueryChange={vi.fn()}
        onAvailabilityFilterChange={vi.fn()}
        onStatusFilterChange={vi.fn()}
        onReviewedFilterChange={vi.fn()}
        onSortChange={vi.fn()}
        onTagIdsChange={vi.fn()}
      />,
    );

    expect(screen.queryByRole("option", { name: "Question A–Z" })).toBeNull();
    expect(screen.getByRole("option", { name: "Recently updated" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Recently studied" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Next scheduled" })).toBeTruthy();
  });
});

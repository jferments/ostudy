import type { ComponentProps } from "react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DeckSummary } from "../../ipc/decks";
import * as folderIpc from "../../ipc/deckFolders";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { CardFilters } from "./CardFilters";

vi.mock("../../ipc/deckFolders", () => ({
  getDeckLibraryOrganization: vi.fn(),
}));

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

function renderFilters(overrides: Partial<ComponentProps<typeof CardFilters>> = {}) {
  const props: ComponentProps<typeof CardFilters> = {
    activeDecks: [deck],
    deckIds: new Set(),
    query: "",
    availabilityFilter: "all",
    statusFilter: "active",
    reviewedFilter: "all",
    sort: "updated_desc",
    tags: [],
    tagIds: new Set(),
    onDeckIdsChange: vi.fn(),
    onQueryChange: vi.fn(),
    onAvailabilityFilterChange: vi.fn(),
    onStatusFilterChange: vi.fn(),
    onReviewedFilterChange: vi.fn(),
    onSortChange: vi.fn(),
    onTagIdsChange: vi.fn(),
    ...overrides,
  };
  render(
    <OverlayProvider>
      <CardFilters {...props} />
    </OverlayProvider>,
  );
  return props;
}

describe("CardFilters", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(folderIpc.getDeckLibraryOrganization).mockResolvedValue({
      revision: 1,
      folders: [],
      placements: [{ deckId: deck.id, folderId: null, sortOrder: 1_000 }],
    });
  });

  it("keeps the ordinary Search view focused on query, scope, sort, and one filter action", () => {
    renderFilters();

    expect(screen.getByRole("searchbox", { name: "Search cards" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /All filters/u })).toBeTruthy();
    expect(screen.queryByRole("combobox", { name: "Availability" })).toBeNull();
    expect(screen.queryByRole("option", { name: "Question A–Z" })).toBeNull();
    expect(screen.getByRole("option", { name: "Recently updated" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Recently studied" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Next scheduled" })).toBeTruthy();
  });

  it("reveals grouped filters in a shared sheet and applies changes immediately", async () => {
    const props = renderFilters();
    fireEvent.click(screen.getByRole("button", { name: /All filters/u }));

    const dialog = await screen.findByRole("dialog", { name: "Filter flashcards" });
    fireEvent.change(within(dialog).getByRole("combobox", { name: "Availability" }), {
      target: { value: "ready_now" },
    });
    expect(props.onAvailabilityFilterChange).toHaveBeenCalledWith("ready_now");
  });

  it("exposes applied filters as directly removable chips", () => {
    const props = renderFilters({ availabilityFilter: "ready_now" });

    fireEvent.click(screen.getByRole("button", { name: "Remove Ready now filter" }));
    expect(props.onAvailabilityFilterChange).toHaveBeenCalledWith("all");
  });
});

import { fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import { LayoutModeProvider } from "../../../ui/layout/LayoutModeProvider";
import { DeckItem } from "./DeckItem";
import type { DeckLibraryReorderAdapter } from "./useDeckLibraryReorder";

const originalMatchMedia = window.matchMedia;
const deck = makeDeck("a", "Anatomy");
const folderId = "f".repeat(32);

describe("DeckItem", () => {
  beforeEach(() => {
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
  });

  it.each(["ArrowUp", "ArrowDown", "Home", "End", "ArrowLeft", "ArrowRight"])(
    "routes %s from the primary deck control through the reorder adapter",
    (key) => {
      const reorder = makeReorder();
      renderDeckItem({ reorder });
      const openControl = screen.getByRole("button", { name: /Anatomy.*cards.*ready/ });

      fireEvent.keyDown(openControl, { key });

      expect(reorder.moveDeckWithKeyboard).toHaveBeenCalledTimes(1);
      expect(reorder.moveDeckWithKeyboard).toHaveBeenCalledWith(
        expect.objectContaining({ key }),
        deck.id,
        folderId,
        2,
        5,
      );
      expect(openControl.getAttribute("aria-keyshortcuts")).toBe(
        "ArrowUp ArrowDown Home End ArrowLeft ArrowRight",
      );
    },
  );

  it("leaves default prevention to the validated adapter and preserves opening", () => {
    const onOpen = vi.fn();
    const reorder = makeReorder();
    vi.mocked(reorder.moveDeckWithKeyboard).mockImplementation((event) => {
      if (event.key === "ArrowDown") event.preventDefault();
    });
    const { container } = renderDeckItem({ reorder, onOpen });
    const openControl = screen.getByRole("button", { name: /Anatomy.*cards.*ready/ });
    const movement = keyboardEvent("ArrowDown");
    const enter = keyboardEvent("Enter");
    const space = keyboardEvent(" ");

    openControl.dispatchEvent(movement);
    openControl.dispatchEvent(enter);
    openControl.dispatchEvent(space);
    fireEvent.click(openControl);

    expect(movement.defaultPrevented).toBe(true);
    expect(enter.defaultPrevented).toBe(false);
    expect(space.defaultPrevented).toBe(false);
    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Move earlier" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Move later" })).toBeTruthy();
    expect(container.querySelector("[data-library-drag-handle]")).toBeNull();
    expect(screen.queryByRole("button", { name: /move anatomy/i })).toBeNull();
  });
});

function renderDeckItem(
  overrides: Partial<ComponentProps<typeof DeckItem>> = {},
) {
  const props: ComponentProps<typeof DeckItem> = {
    deck,
    folderId,
    index: 2,
    count: 5,
    disabled: false,
    selected: false,
    reorder: makeReorder(),
    onOpen: vi.fn(),
    onPractice: vi.fn(),
    onDelete: vi.fn(),
    onToggleSelected: vi.fn(),
    ...overrides,
  };
  return render(
    <LayoutModeProvider>
      <DeckItem {...props} />
    </LayoutModeProvider>,
  );
}

function makeReorder(): DeckLibraryReorderAdapter {
  return {
    moveDeckWithKeyboard: vi.fn(),
    moveFolderWithKeyboard: vi.fn(),
    moveDeckWithFallback: vi.fn(),
    beginPointerDrag: vi.fn(),
  };
}

function keyboardEvent(key: string): globalThis.KeyboardEvent {
  return new window.KeyboardEvent("keydown", {
    key,
    bubbles: true,
    cancelable: true,
  });
}

function makeDeck(seed: string, name: string): DeckSummary {
  return {
    id: seed.repeat(32),
    name,
    description: "",
    sourceNoteCount: 1,
    generatedCardCount: 12,
    studyableDueCount: 4,
    archivedAt: null,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  };
}

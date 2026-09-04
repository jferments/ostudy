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
    setLayoutMode("wide");
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
  });

  it.each(["ArrowUp", "ArrowDown", "Home", "End", "ArrowLeft", "ArrowRight"])(
    "routes %s from the keyboard-reachable deck row through the reorder adapter",
    (key) => {
      const reorder = makeReorder();
      renderDeckItem({ reorder });
      const openSurface = screen.getByRole("button", { name: "Open Anatomy" });

      fireEvent.keyDown(openSurface, { key });

      expect(reorder.moveDeckWithKeyboard).toHaveBeenCalledTimes(1);
      expect(reorder.moveDeckWithKeyboard).toHaveBeenCalledWith(
        expect.objectContaining({ key }),
        deck.id,
        folderId,
        2,
        5,
      );
      expect(openSurface.getAttribute("aria-keyshortcuts")).toBe(
        "Enter Space ArrowUp ArrowDown Home End ArrowLeft ArrowRight",
      );
      expect(openSurface.getAttribute("data-library-pointer-drag-source")).toBe("true");
    },
  );

  it.each(["wide", "medium", "compact"] as const)(
    "starts pointer drag from the broad title and copy surface but not controls in %s layout",
    (layoutMode) => {
      setLayoutMode(layoutMode);
      const reorder = makeReorder();
      renderDeckItem({ reorder });

      fireEvent.pointerDown(screen.getByText("Anatomy"));
      fireEvent.pointerDown(screen.getByText("12 cards · 4 ready"));

      expect(reorder.beginPointerDrag).toHaveBeenCalledTimes(2);
      expect(reorder.beginPointerDrag).toHaveBeenLastCalledWith(
        expect.anything(),
        { kind: "deck", id: deck.id },
        deck.name,
      );

      fireEvent.pointerDown(screen.getByRole("checkbox", { name: "Select Anatomy" }));
      fireEvent.pointerDown(screen.getByRole("button", { name: "Practice" }));
      fireEvent.pointerDown(screen.getByRole("button", { name: "Delete Anatomy" }));

      expect(reorder.beginPointerDrag).toHaveBeenCalledTimes(2);
    },
  );

  it.each(["wide", "medium", "compact"] as const)(
    "opens from short clicks on the deck row while controls stay isolated in %s layout",
    (layoutMode) => {
      setLayoutMode(layoutMode);
      const onOpen = vi.fn();
      const onToggleSelected = vi.fn();
      renderDeckItem({ onOpen, onToggleSelected });

      const title = screen.getByText("Anatomy");
      const copy = screen.getByText("12 cards · 4 ready");
      const openSurface = screen.getByRole("button", { name: "Open Anatomy" });
      const selection = screen.getByRole("checkbox", { name: "Select Anatomy" });

      expect(openSurface.tagName).toBe("DIV");
      expect(openSurface.getAttribute("tabindex")).toBe("0");
      expect(screen.queryByText(/^Open$/u)).toBeNull();

      fireEvent.click(title);
      fireEvent.click(copy);
      fireEvent.click(openSurface);

      expect(onOpen).toHaveBeenCalledTimes(3);
      expect(onToggleSelected).not.toHaveBeenCalled();
      expect((selection as HTMLInputElement).checked).toBe(false);

      fireEvent.click(selection);

      expect(onOpen).toHaveBeenCalledTimes(3);
      expect(onToggleSelected).toHaveBeenCalledTimes(1);
    },
  );

  it("keeps the trash action direct in compact layout instead of creating a one-item menu", () => {
    setLayoutMode("compact");
    const onDelete = vi.fn();
    renderDeckItem({ onDelete });

    expect(screen.queryByRole("button", { name: "Anatomy actions" })).toBeNull();
    const trash = screen.getByRole("button", { name: "Delete Anatomy" });
    expect(trash.getAttribute("data-tone")).toBe("danger");
    fireEvent.click(trash);
    expect(onDelete).toHaveBeenCalledOnce();
  });

  it("opens with Enter and Space while leaving movement prevention to the reorder adapter", () => {
    const onOpen = vi.fn();
    const reorder = makeReorder();
    vi.mocked(reorder.moveDeckWithKeyboard).mockImplementation((event) => {
      if (event.key === "ArrowDown") event.preventDefault();
    });
    const { container } = renderDeckItem({ reorder, onOpen });
    const openSurface = screen.getByRole("button", { name: "Open Anatomy" });
    const movement = keyboardEvent("ArrowDown");
    const enter = keyboardEvent("Enter");
    const space = keyboardEvent(" ");

    openSurface.dispatchEvent(movement);
    openSurface.dispatchEvent(enter);
    openSurface.dispatchEvent(space);
    fireEvent.click(openSurface);

    expect(movement.defaultPrevented).toBe(true);
    expect(enter.defaultPrevented).toBe(true);
    expect(space.defaultPrevented).toBe(true);
    expect(onOpen).toHaveBeenCalledTimes(3);
    expect(screen.getByRole("button", { name: "Move earlier" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Move later" })).toBeTruthy();
    expect(container.querySelector("[data-library-drag-handle]")).toBeNull();
    expect(screen.queryByRole("button", { name: /move anatomy/i })).toBeNull();
  });
});

function renderDeckItem(overrides: Partial<ComponentProps<typeof DeckItem>> = {}) {
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

function setLayoutMode(layoutMode: "wide" | "medium" | "compact"): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches:
      (query === "(max-width: 760px)" && layoutMode === "compact") ||
      (query === "(max-width: 1180px)" && layoutMode !== "wide"),
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
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

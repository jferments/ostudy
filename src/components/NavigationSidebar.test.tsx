import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { LayoutModeProvider } from "../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { NavigationSidebar } from "./NavigationSidebar";

const originalMatchMedia = window.matchMedia;

afterEach(() => {
  window.matchMedia = originalMatchMedia;
  delete document.documentElement.dataset.layoutMode;
  delete document.documentElement.dataset.inputMode;
});

function setViewport(width: number, coarsePointer = false): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches:
      (query.includes("max-width: 760px") && width <= 760) ||
      (query.includes("max-width: 1180px") && width <= 1180) ||
      (query.includes("pointer: coarse") && coarsePointer),
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

function renderNavigation(
  width: number,
  onNavigate = vi.fn(),
  section: "decks" | "study-guides" | "documents" | "practice" | "analytics" | "settings" = "decks",
) {
  setViewport(width);
  render(
    <LayoutModeProvider>
      <OverlayProvider>
        <NavigationSidebar section={section} onNavigate={onNavigate} />
      </OverlayProvider>
    </LayoutModeProvider>,
  );
  return onNavigate;
}

describe("NavigationSidebar", () => {
  it("renders every primary section directly in wide layouts", () => {
    renderNavigation(1440);

    const navigation = screen.getByRole("navigation", { name: "Main navigation" });
    expect(within(navigation).getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Flashcards",
      "Study Guides",
      "Documents",
      "Analytics",
      "Settings",
    ]);
    expect(navigation).toBeTruthy();
    expect(document.documentElement.dataset.layoutMode).toBe("wide");
    expect(screen.queryByRole("button", { name: "Search/Filter" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Templates" })).toBeNull();
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Practice" })).toBeNull();
  });

  it("keeps labeled primary navigation in the medium tablet layout", () => {
    renderNavigation(1000);

    expect(document.documentElement.dataset.layoutMode).toBe("medium");
    const navigation = screen.getByRole("navigation", { name: "Main navigation" });
    expect(within(navigation).getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Flashcards",
      "Study Guides",
      "Documents",
      "Analytics",
      "Settings",
    ]);
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Practice" })).toBeNull();
  });

  it("routes every visible destination through the typed navigation callback", () => {
    const onNavigate = renderNavigation(1440);
    const navigation = screen.getByRole("navigation", { name: "Main navigation" });

    for (const label of [
      "Flashcards",
      "Study Guides",
      "Documents",
      "Analytics",
      "Settings",
    ]) {
      fireEvent.click(within(navigation).getByRole("button", { name: label }));
    }

    expect(onNavigate.mock.calls.map(([section]) => section)).toEqual([
      "decks",
      "study-guides",
      "documents",
      "analytics",
      "settings",
    ]);
  });

  it("partitions compact destinations once and restores focus after dismissing More", async () => {
    const onNavigate = renderNavigation(390);

    expect(document.documentElement.dataset.layoutMode).toBe("compact");
    const navigation = screen.getByRole("navigation", { name: "Main navigation" });
    expect(within(navigation).getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Flashcards",
      "Study Guides",
      "Analytics",
      "More",
    ]);
    expect(screen.queryByRole("button", { name: "Documents" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Templates" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Practice" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Analytics" }));
    expect(onNavigate).toHaveBeenCalledWith("analytics");

    const moreButton = screen.getByRole("button", { name: "More" });
    moreButton.focus();
    fireEvent.click(moreButton);
    const dialog = await screen.findByRole("dialog", { name: "More" });
    expect(moreButton.getAttribute("aria-expanded")).toBe("true");
    expect(within(dialog).getAllByRole("button").map((button) => button.textContent)).toEqual([
      "×",
      "Documents",
      "Settings",
    ]);
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "More" })).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(moreButton));
    expect(moreButton.getAttribute("aria-expanded")).toBe("false");

    fireEvent.click(moreButton);
    const reopenedDialog = await screen.findByRole("dialog", { name: "More" });
    fireEvent.click(within(reopenedDialog).getByRole("button", { name: "Documents" }));
    expect(onNavigate).toHaveBeenCalledWith("documents");
  });

  it("marks only visible destinations active and leaves internal Practice unselected", async () => {
    renderNavigation(390, vi.fn(), "documents");

    const more = screen.getByRole("button", { name: "More" });
    expect(more.getAttribute("aria-current")).toBe("page");
    fireEvent.click(more);
    expect(
      (await screen.findByRole("button", { name: "Documents" })).getAttribute("aria-current"),
    ).toBe("page");

    fireEvent.click(screen.getByRole("button", { name: "Close dialog" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "More" })).toBeNull());
  });

  it("does not select More for the internal Practice workflow", () => {
    renderNavigation(390, vi.fn(), "practice");

    expect(screen.getByRole("button", { name: "More" }).getAttribute("aria-current")).toBeNull();
    expect(screen.queryByRole("button", { name: "Practice" })).toBeNull();
  });
});

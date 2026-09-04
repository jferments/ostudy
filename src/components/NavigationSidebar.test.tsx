import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { AppSection, AppWorkflow } from "../controllers/useAppController";
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
  section: AppSection = "decks",
  activeWorkflow: AppWorkflow | null = null,
) {
  setViewport(width);
  render(
    <LayoutModeProvider>
      <OverlayProvider>
        <NavigationSidebar
          section={section}
          activeWorkflow={activeWorkflow}
          onNavigate={onNavigate}
        />
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
  });

  it("routes every wide destination through the typed navigation callback", () => {
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

  it("renders the four requested destinations directly in compact bottom navigation", () => {
    const onNavigate = renderNavigation(390);

    expect(document.documentElement.dataset.layoutMode).toBe("compact");
    const navigation = screen.getByRole("navigation", { name: "Main navigation" });
    expect(within(navigation).getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Flashcards",
      "Study Guides",
      "Documents",
      "Analytics",
    ]);
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Settings" })).toBeNull();

    fireEvent.click(within(navigation).getByRole("button", { name: "Documents" }));
    expect(onNavigate).toHaveBeenCalledWith("documents");
  });

  it("marks the direct compact destination active", () => {
    renderNavigation(390, vi.fn(), "documents");

    expect(screen.getByRole("button", { name: "Documents" }).getAttribute("aria-current")).toBe(
      "page",
    );
    expect(screen.getByRole("button", { name: "Flashcards" }).getAttribute("aria-current")).toBeNull();
  });

  it("leaves every navigation destination unselected during the internal Practice workflow", () => {
    renderNavigation(390, vi.fn(), "decks", "practice");

    const navigation = screen.getByRole("navigation", { name: "Main navigation" });
    for (const button of within(navigation).getAllByRole("button")) {
      expect(button.getAttribute("aria-current")).toBeNull();
    }
    expect(screen.queryByRole("button", { name: "Practice" })).toBeNull();
  });

  it("renders no app-owned window controls in desktop or compact navigation", () => {
    renderNavigation(1440);
    expect(screen.queryByRole("button", { name: /maximize|restore window|full screen/iu })).toBeNull();

    renderNavigation(390);
    expect(screen.queryByRole("button", { name: /maximize|restore window|full screen/iu })).toBeNull();
  });
});

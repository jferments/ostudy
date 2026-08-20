import { fireEvent, render, screen } from "@testing-library/react";
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

function renderNavigation(width: number, onNavigate = vi.fn()) {
  setViewport(width);
  render(
    <LayoutModeProvider>
      <OverlayProvider>
        <NavigationSidebar section="decks" onNavigate={onNavigate} />
      </OverlayProvider>
    </LayoutModeProvider>,
  );
  return onNavigate;
}

describe("NavigationSidebar", () => {
  it("renders every primary section directly in wide layouts", () => {
    renderNavigation(1440);

    const navigation = screen.getByRole("navigation", { name: "Main navigation" });
    for (const label of ["Flashcards", "Documents", "Practice", "Analytics", "Settings"]) {
      expect(screen.getByRole("button", { name: label })).toBeTruthy();
    }
    expect(navigation).toBeTruthy();
    expect(document.documentElement.dataset.layoutMode).toBe("wide");
    expect(screen.queryByRole("button", { name: "Search/Filter" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Templates" })).toBeNull();
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
  });

  it("keeps labeled primary navigation in the medium tablet layout", () => {
    renderNavigation(1000);

    expect(document.documentElement.dataset.layoutMode).toBe("medium");
    for (const label of ["Flashcards", "Documents", "Practice", "Analytics", "Settings"]) {
      expect(screen.getByRole("button", { name: label })).toBeTruthy();
    }
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
  });

  it("preserves compact priorities and exposes Documents through More", async () => {
    const onNavigate = renderNavigation(390);

    expect(document.documentElement.dataset.layoutMode).toBe("compact");
    for (const label of ["Flashcards", "Practice", "Analytics", "More"]) {
      expect(screen.getByRole("button", { name: label })).toBeTruthy();
    }
    expect(screen.queryByRole("button", { name: "Documents" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Templates" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Analytics" }));
    expect(onNavigate).toHaveBeenCalledWith("analytics");

    fireEvent.click(screen.getByRole("button", { name: "More" }));
    expect(await screen.findByRole("dialog", { name: "More" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Documents" }));
    expect(onNavigate).toHaveBeenCalledWith("documents");
  });
});

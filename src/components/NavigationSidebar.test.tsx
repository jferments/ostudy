import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { NavigationSidebar } from "./NavigationSidebar";

const originalMatchMedia = window.matchMedia;

afterEach(() => {
  window.matchMedia = originalMatchMedia;
});

function setCompact(compact: boolean): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: compact && query.includes("max-width: 760px"),
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

function renderNavigation(compact: boolean, onNavigate = vi.fn()) {
  setCompact(compact);
  render(
    <OverlayProvider>
      <NavigationSidebar section="decks" onNavigate={onNavigate} />
    </OverlayProvider>,
  );
  return onNavigate;
}

describe("NavigationSidebar", () => {
  it("renders every primary section directly in wide layouts", () => {
    renderNavigation(false);

    const navigation = screen.getByRole("navigation", { name: "Main navigation" });
    for (const label of ["Flashcards", "Documents", "Practice", "Analytics", "Settings"]) {
      expect(screen.getByRole("button", { name: label })).toBeTruthy();
    }
    expect(navigation).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Search/Filter" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Templates" })).toBeNull();
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
  });

  it("preserves compact priorities and exposes Documents through More", async () => {
    const onNavigate = renderNavigation(true);

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

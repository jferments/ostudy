import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { NavigationSidebar } from "./NavigationSidebar";

const appearanceMocks = vi.hoisted(() => ({
  mode: "wide" as "compact" | "medium" | "wide",
}));

vi.mock("../ui/appearance/AppearanceProvider", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../ui/appearance/AppearanceProvider")>();
  return {
    ...actual,
    useLayoutMode: () => appearanceMocks.mode,
  };
});

function renderNavigation(
  mode: "compact" | "medium" | "wide",
  onNavigate = vi.fn(),
) {
  appearanceMocks.mode = mode;
  render(
    <OverlayProvider>
      <NavigationSidebar section="decks" onNavigate={onNavigate} />
    </OverlayProvider>,
  );
  return onNavigate;
}

describe("NavigationSidebar", () => {
  it("renders every primary section directly in wide layouts", () => {
    renderNavigation("wide");

    const navigation = screen.getByRole("navigation", { name: "Main navigation" });
    for (const label of ["Flashcards", "Documents", "Practice", "Analytics", "Settings"]) {
      expect(screen.getByRole("button", { name: label })).toBeTruthy();
    }
    expect(navigation).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Search/Filter" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Templates" })).toBeNull();
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
  });

  it("keeps every primary destination labeled in medium tablet layouts", () => {
    renderNavigation("medium");

    const navigation = screen.getByRole("navigation", { name: "Main navigation" });
    for (const label of ["Flashcards", "Documents", "Practice", "Analytics", "Settings"]) {
      const button = withinNavigation(navigation, label);
      expect(button.textContent).toContain(label);
    }
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
  });

  it("preserves compact priorities and exposes Documents through More", async () => {
    const onNavigate = renderNavigation("compact");

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

function withinNavigation(navigation: HTMLElement, label: string): HTMLButtonElement {
  const button = [...navigation.querySelectorAll<HTMLButtonElement>("button")].find(
    (candidate) => candidate.textContent?.trim() === label,
  );
  if (!button) throw new Error(`Missing ${label} navigation button.`);
  return button;
}

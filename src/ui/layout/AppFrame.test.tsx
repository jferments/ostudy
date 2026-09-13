import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LayoutModeProvider } from "./LayoutModeProvider";
import { AppFrame, AppContent } from "./AppFrame";
import { AppNavigation } from "./AppNavigation";
const media = window.matchMedia;
afterEach(() => { window.matchMedia = media; });
describe("shadcn application frame", () => {
  it.each([390, 1000, 1440])("preserves one main scroll area and named navigation at %s px", (width) => {
    window.matchMedia = vi.fn((query) => ({ matches: query.includes("760") ? width <= 760 : query.includes("1180") ? width <= 1180 : false, media: query, addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as MediaQueryList));
    const view = render(<LayoutModeProvider><AppFrame><AppNavigation brand={<span>ostudy</span>} items={[{ id: "decks", label: "Flashcards", icon: "cards", active: true, onSelect: vi.fn() }, { id: "settings", label: "Settings", icon: "settings", active: false, secondary: true, onSelect: vi.fn() }]} /><AppContent><h1>Library</h1></AppContent></AppFrame></LayoutModeProvider>);
    expect(screen.getAllByRole("main")).toHaveLength(1);
    expect(screen.getByRole("navigation", { name: "Main navigation" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Flashcards" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByRole("main").getAttribute("data-slot")).toBe("sidebar-inset");
    expect(view.container.querySelector('[data-frame-layout]')?.getAttribute('data-frame-layout')).toBe(width <= 760 ? "compact" : width <= 1180 ? "medium" : "wide");
    expect(view.container.querySelector('[data-slot="bottom-navigation"]') !== null).toBe(width <= 760);
    if (width <= 760) {
      const compactButton = screen.getByRole("button", { name: "Flashcards" });
      expect(compactButton.className).toContain("tw:min-h-[4.25rem]");
      expect(compactButton.className).toContain("tw:overflow-visible");
      expect(compactButton.className).toContain("tw:text-[0.6rem]");
      expect(compactButton.className).toContain("tw:rounded-md");
      expect(view.container.querySelector('[data-slot="bottom-navigation"]')?.className).toContain("tw:pb-[max(0.5rem,var(--safe-bottom))]");
    }
  });
});

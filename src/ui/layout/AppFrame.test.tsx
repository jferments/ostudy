import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LayoutModeProvider } from "./LayoutModeProvider";
import { AppFrame, AppContent } from "./AppFrame";
import { AppNavigation } from "./AppNavigation";
import { matchesLayoutQuery } from "../../test/viewport";
import { viewportBreakpoints } from "../tokens/generated/viewport";
const media = window.matchMedia;
afterEach(() => { window.matchMedia = media; });
describe("shadcn application frame", () => {
  it.each([390, 1000, 1440])("preserves one main scroll area and named navigation at %s px", (width) => {
    window.matchMedia = vi.fn((query) => ({ matches: matchesLayoutQuery(query, width), media: query, addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as MediaQueryList));
    const view = render(<LayoutModeProvider><AppFrame><AppNavigation brand={<span>ostudy</span>} items={[{ id: "decks", label: "Flashcards", icon: "cards", active: true, onSelect: vi.fn() }, { id: "settings", label: "Settings", icon: "settings", active: false, secondary: true, onSelect: vi.fn() }]} /><AppContent><h1>Library</h1></AppContent></AppFrame></LayoutModeProvider>);
    expect(screen.getAllByRole("main")).toHaveLength(1);
    expect(screen.getByRole("navigation", { name: "Main navigation" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Flashcards" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByRole("main").getAttribute("data-slot")).toBe("sidebar-inset");
    expect(view.container.querySelector('[data-frame-layout]')?.getAttribute('data-frame-layout')).toBe(width < viewportBreakpoints.medium ? "compact" : width < viewportBreakpoints.wide ? "medium" : "wide");
    expect(view.container.querySelector('[data-slot="bottom-navigation"]') !== null).toBe(width < viewportBreakpoints.medium);
    if (width < viewportBreakpoints.medium) {
      const compactButton = screen.getByRole("button", { name: "Flashcards" });
      expect(compactButton.className).toContain("tw:min-h-[3.85rem]");
      expect(compactButton.className).toContain("tw:overflow-visible");
      expect(compactButton.className).toContain("tw:text-[0.55rem]");
      expect(compactButton.className).toContain("tw:rounded-sm");
      expect(compactButton.className).toContain("tw:coarse:px-0.5");
      expect(view.container.querySelector('[data-slot="bottom-navigation"]')?.className).toContain("tw:pb-[max(0.5rem,var(--safe-bottom))]");
    }
  });
});

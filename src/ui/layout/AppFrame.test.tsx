import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { matchesLayoutQuery } from "../../test/viewport";
import { viewportBreakpoints } from "../tokens/generated/viewport";
import { AppContent, AppFrame, AppHeader } from "./AppFrame";
import { AppNavigation, type NavigationItem } from "./AppNavigation";
import { LayoutModeProvider } from "./LayoutModeProvider";

const media = window.matchMedia;
afterEach(() => {
  window.matchMedia = media;
});

function setViewport(width: number): void {
  window.matchMedia = vi.fn(
    (query: string) =>
      ({
        matches: matchesLayoutQuery(query, width),
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      }) as unknown as MediaQueryList,
  );
}

function items(onSelect = vi.fn()): NavigationItem[] {
  return [
    { id: "home", label: "Home", icon: "home", active: true, onSelect },
    {
      id: "review",
      label: "Review",
      icon: "practice",
      active: false,
      badge: { value: "12", description: "12 cards due today" },
      onSelect,
    },
  ];
}

function renderFrame(width: number, onSelect = vi.fn()) {
  setViewport(width);
  const navigation = <AppNavigation items={items(onSelect)} />;
  const compact = width < viewportBreakpoints.medium;
  return render(
    <LayoutModeProvider>
      <AppFrame>
        <AppHeader
          brand={<span>ostudy</span>}
          title={compact ? "Library" : undefined}
          navigation={compact ? undefined : navigation}
          actions={<button type="button">Settings</button>}
        />
        {compact ? navigation : null}
        <AppContent>
          <h1>Home</h1>
        </AppContent>
      </AppFrame>
    </LayoutModeProvider>,
  );
}

describe("application frame", () => {
  it.each([390, 1000, 1440])("keeps one main scroll area and the named navigation at %s px", (width) => {
    const view = renderFrame(width);
    expect(screen.getAllByRole("main")).toHaveLength(1);
    expect(screen.getByRole("main").getAttribute("data-slot")).toBe("sidebar-inset");
    expect(screen.getByRole("navigation", { name: "Main navigation" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Home" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByRole("button", { name: "Review" }).getAttribute("aria-current")).toBeNull();
    const layout =
      width < viewportBreakpoints.medium ? "compact" : width < viewportBreakpoints.wide ? "medium" : "wide";
    expect(view.container.querySelector("[data-frame-layout]")?.getAttribute("data-frame-layout")).toBe(layout);
    const bottom = view.container.querySelector('[data-slot="bottom-navigation"]');
    const header = view.container.querySelector('[data-slot="app-header"]');
    expect(bottom !== null).toBe(layout === "compact");
    // Wider screens carry the navigation inside the header.
    expect(header?.querySelector('[data-slot="header-navigation"]') !== null).toBe(layout !== "compact");
  });

  it("keeps the visible label as each destination's name and describes the badge", () => {
    for (const width of [390, 1440]) {
      const view = renderFrame(width);
      const review = screen.getByRole("button", { name: "Review" });
      expect(review.getAttribute("data-testid")).toBe("nav-review");
      const description = document.getElementById(review.getAttribute("aria-describedby") ?? "");
      expect(description?.textContent).toBe("12 cards due today");
      expect(within(review).getByText("12").getAttribute("aria-hidden")).toBe("true");
      expect(screen.getByRole("button", { name: "Home" }).hasAttribute("aria-describedby")).toBe(false);
      view.unmount();
    }
  });

  it("draws the phone's bottom bar with full-size targets and the safe area below", () => {
    const view = renderFrame(390);
    const bottom = view.container.querySelector('[data-slot="bottom-navigation"]');
    expect(bottom?.className).toContain("tw:pb-[max(0.5rem,var(--safe-bottom))]");
    expect(bottom?.querySelector("ul")?.className).toContain("tw:grid-cols-5");
    const home = screen.getByRole("button", { name: "Home" });
    expect(home.className).toContain("tw:min-h-[3.625rem]");
    expect(home.className).toContain("tw:coarse:min-h-touch");
    expect(home.className).toContain("tw:text-caption");
    const header = view.container.querySelector('[data-slot="app-header"]');
    expect(header?.className).toContain("tw:fixed");
    expect(header?.className).toContain("tw:pt-(--safe-top)");
    expect(within(header as HTMLElement).getByText("Library")).toBeTruthy();
  });

  it("routes a selection through the item's callback", () => {
    const onSelect = vi.fn();
    renderFrame(1440, onSelect);
    fireEvent.click(screen.getByRole("button", { name: "Review" }));
    expect(onSelect).toHaveBeenCalledTimes(1);
  });
});

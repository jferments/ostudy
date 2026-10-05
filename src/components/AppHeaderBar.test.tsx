import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { AppSection, AppWorkflow } from "../controllers/useAppController";
import { matchesLayoutQuery } from "../test/viewport";
import { LayoutModeProvider } from "../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { AppHeaderBar } from "./AppHeaderBar";

const originalMatchMedia = window.matchMedia;

afterEach(() => {
  window.matchMedia = originalMatchMedia;
  delete document.documentElement.dataset.layoutMode;
});

function setViewport(width: number): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: matchesLayoutQuery(query, width),
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

function renderHeader({
  width = 1440,
  section = "home",
  activeWorkflow = null,
}: { width?: number; section?: AppSection; activeWorkflow?: AppWorkflow | null } = {}) {
  setViewport(width);
  const onSearch = vi.fn();
  const onSettings = vi.fn();
  const view = render(
    <LayoutModeProvider>
      <OverlayProvider>
        <AppHeaderBar
          section={section}
          activeWorkflow={activeWorkflow}
          navigation={<nav aria-label="Main navigation" />}
          onSearch={onSearch}
          onSettings={onSettings}
        />
      </OverlayProvider>
    </LayoutModeProvider>,
  );
  return { ...view, onSearch, onSettings };
}

describe("AppHeaderBar", () => {
  it.each([390, 1000, 1440])("offers Search and the Settings gear without window controls at %s px", (width) => {
    const { onSearch, onSettings } = renderHeader({ width });
    const header = screen.getByTestId("app-header");
    fireEvent.click(within(header).getByRole("button", { name: "Search" }));
    fireEvent.click(within(header).getByRole("button", { name: "Settings" }));
    expect(onSearch).toHaveBeenCalledTimes(1);
    expect(onSettings).toHaveBeenCalledTimes(1);
    expect(within(header).getByTestId("app-settings")).toBeTruthy();
    expect(within(header).getByTestId("app-search")).toBeTruthy();
    expect(
      within(header).queryByRole("button", { name: /maximize|restore window|full screen/iu }),
    ).toBeNull();
  });

  it("carries the navigation inside the header on wider screens and below it on a phone", () => {
    const wide = renderHeader({ width: 1440 });
    expect(within(screen.getByTestId("app-header")).getByRole("navigation", { name: "Main navigation" })).toBeTruthy();
    wide.unmount();

    renderHeader({ width: 390 });
    expect(within(screen.getByTestId("app-header")).queryByRole("navigation")).toBeNull();
    expect(screen.getByRole("navigation", { name: "Main navigation" })).toBeTruthy();
  });

  it.each([
    ["home", null, null],
    ["decks", null, "Library"],
    ["documents", null, "Library"],
    ["study-guides", null, "Library"],
    ["review", null, "Review"],
    ["tutor", null, "Tutor"],
    ["analytics", null, "Progress"],
    ["settings", null, "Settings"],
    ["decks", "practice", "Practice"],
  ] as const)("titles a phone's %s screen (workflow %s) as %s", (section, activeWorkflow, title) => {
    renderHeader({ width: 390, section, activeWorkflow });
    const header = screen.getByTestId("app-header");
    for (const candidate of ["Library", "Review", "Tutor", "Progress", "Practice"]) {
      if (candidate !== title) expect(within(header).queryByText(candidate)).toBeNull();
    }
    if (title) expect(within(header).getByText(title)).toBeTruthy();
  });

  it("marks the gear current only on Settings itself", () => {
    const settings = renderHeader({ section: "settings" });
    expect(screen.getByRole("button", { name: "Settings" }).getAttribute("aria-current")).toBe("page");
    settings.unmount();

    const practice = renderHeader({ section: "settings", activeWorkflow: "practice" });
    expect(screen.getByRole("button", { name: "Settings" }).getAttribute("aria-current")).toBeNull();
    practice.unmount();

    renderHeader({ section: "documents" });
    expect(screen.getByRole("button", { name: "Settings" }).getAttribute("aria-current")).toBeNull();
  });

  it("shows the brand: the lockup on wide screens and phones, the mark alone at medium width", () => {
    const wide = renderHeader({ width: 1440 });
    expect(wide.container.querySelector('[data-artwork-slot="brand.logo.sidebar"]')).not.toBeNull();
    wide.unmount();

    const medium = renderHeader({ width: 1000 });
    expect(medium.container.querySelector('[data-artwork-slot="brand.logo.sidebar"]')).toBeNull();
    expect(medium.container.querySelector('[data-artwork-slot="brand.logo.compact"]')).not.toBeNull();
    medium.unmount();

    const phone = renderHeader({ width: 390 });
    expect(phone.container.querySelector('[data-artwork-slot="brand.logo.sidebar"]')).not.toBeNull();
  });
});

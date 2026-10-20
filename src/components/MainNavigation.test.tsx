import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi, type Mock } from "vitest";

import type { AppSection, AppWorkflow } from "../controllers/useAppController";
import type { AppDestination } from "../navigation/appDestinations";
import { matchesLayoutQuery } from "../test/viewport";
import { LayoutModeProvider } from "../ui/layout/LayoutModeProvider";
import { dueTodayBadge, MainNavigation } from "./MainNavigation";

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

function renderNavigation({
  width = 1440,
  section = "home",
  activeWorkflow = null,
  dueToday = null,
  onSelect = vi.fn<(destination: AppDestination) => void>(),
}: {
  width?: number;
  section?: AppSection;
  activeWorkflow?: AppWorkflow | null;
  dueToday?: number | null;
  onSelect?: Mock<(destination: AppDestination) => void>;
} = {}) {
  setViewport(width);
  const view = render(
    <LayoutModeProvider>
      <MainNavigation
        section={section}
        activeWorkflow={activeWorkflow}
        dueToday={dueToday}
        onSelect={onSelect}
      />
    </LayoutModeProvider>,
  );
  return { ...view, onSelect };
}

function buttons() {
  const navigation = screen.getByRole("navigation", { name: "Main navigation" });
  return within(navigation).getAllByRole("button");
}

function current(): string[] {
  return buttons()
    .filter((button) => button.getAttribute("aria-current") === "page")
    .map((button) => button.getAttribute("data-testid") ?? "");
}

describe("MainNavigation", () => {
  it.each([1000, 1440])("names Home, Library, Tutor and Progress at %s px, with Settings at the rail's foot", (width) => {
    renderNavigation({ width });
    expect(buttons().map((button) => button.querySelector('[data-slot="nav-label"]')?.textContent)).toEqual([
      "Home",
      "Library",
      "Tutor",
      "Progress",
    ]);
    expect(screen.queryByRole("button", { name: "Settings" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Review" })).toBeNull();
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Practice" })).toBeNull();
    expect(screen.queryByRole("button", { name: /maximize|restore window|full screen/iu })).toBeNull();
  });

  it("ends a phone's bottom bar with Settings", () => {
    renderNavigation({ width: 390 });
    expect(buttons().map((button) => button.querySelector('[data-slot="nav-label"]')?.textContent)).toEqual([
      "Home",
      "Library",
      "Tutor",
      "Progress",
      "Settings",
    ]);
    expect(screen.getByRole("button", { name: "Settings" }).getAttribute("data-testid")).toBe("app-settings");
  });

  it("routes each destination through the typed callback", () => {
    const { onSelect } = renderNavigation({ width: 390 });
    for (const label of ["Home", "Library", "Tutor", "Progress", "Settings"]) {
      fireEvent.click(screen.getByRole("button", { name: label }));
    }
    expect(onSelect.mock.calls.map(([destination]) => destination)).toEqual([
      "home",
      "library",
      "tutor",
      "progress",
      "settings",
    ]);
  });

  it.each([
    ["home", "nav-home"],
    ["decks", "nav-library"],
    ["documents", "nav-library"],
    ["study-guides", "nav-library"],
    ["review", "nav-home"],
    ["tutor", "nav-tutor"],
    ["analytics", "nav-progress"],
  ] as const)("highlights the destination that holds %s", (section, testId) => {
    renderNavigation({ section, width: 390 });
    expect(current()).toEqual([testId]);
  });

  it("highlights Settings in a phone's bottom bar and nothing in the rail, where Settings is at the foot", () => {
    const view = renderNavigation({ section: "settings", width: 390 });
    expect(current()).toEqual(["app-settings"]);
    view.unmount();
    renderNavigation({ section: "settings" });
    expect(current()).toEqual([]);
  });

  it("highlights Home while a practice session runs, wherever it started", () => {
    for (const section of ["decks", "analytics", "home"] as const) {
      const view = renderNavigation({ section, activeWorkflow: "practice" });
      expect(current()).toEqual(["nav-home"]);
      view.unmount();
    }
  });

  it("shows today's count on Home without changing its name", () => {
    renderNavigation({ dueToday: 47 });
    const review = screen.getByRole("button", { name: "Home" });
    expect(within(review).getByText("47")).toBeTruthy();
    expect(document.getElementById(review.getAttribute("aria-describedby") ?? "")?.textContent).toBe(
      "47 cards due today",
    );
  });

  it("shows no badge while the count is unknown or zero", () => {
    for (const dueToday of [null, 0]) {
      const view = renderNavigation({ dueToday });
      expect(view.container.querySelector('[data-slot="nav-badge"]')).toBeNull();
      expect(screen.getByRole("button", { name: "Home" }).hasAttribute("aria-describedby")).toBe(false);
      view.unmount();
    }
  });

  it("caps the badge and keeps the exact count in its description", () => {
    expect(dueTodayBadge(1)).toEqual({ value: "1", description: "1 card due today" });
    expect(dueTodayBadge(1234)).toEqual({ value: "999+", description: "1,234 cards due today" });
  });
});

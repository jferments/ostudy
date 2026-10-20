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
  it.each([390, 1000, 1440])("names the same five destinations in order at %s px", (width) => {
    renderNavigation({ width });
    expect(buttons().map((button) => button.querySelector('[data-slot="nav-label"]')?.textContent)).toEqual([
      "Home",
      "Library",
      "Review",
      "Tutor",
      "Progress",
    ]);
    expect(screen.queryByRole("button", { name: "Settings" })).toBeNull();
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Practice" })).toBeNull();
    expect(screen.queryByRole("button", { name: /maximize|restore window|full screen/iu })).toBeNull();
  });

  it("routes each destination through the typed callback", () => {
    const { onSelect } = renderNavigation();
    for (const label of ["Home", "Library", "Review", "Tutor", "Progress"]) {
      fireEvent.click(screen.getByRole("button", { name: label }));
    }
    expect(onSelect.mock.calls.map(([destination]) => destination)).toEqual([
      "home",
      "library",
      "review",
      "tutor",
      "progress",
    ]);
  });

  it.each([
    ["home", "nav-home"],
    ["decks", "nav-library"],
    ["documents", "nav-library"],
    ["study-guides", "nav-library"],
    ["review", "nav-review"],
    ["tutor", "nav-tutor"],
    ["analytics", "nav-progress"],
  ] as const)("highlights the destination that holds %s", (section, testId) => {
    renderNavigation({ section, width: 390 });
    expect(current()).toEqual([testId]);
  });

  it("highlights no destination on Settings, which lives in the header", () => {
    renderNavigation({ section: "settings" });
    expect(current()).toEqual([]);
  });

  it("highlights Review while a practice session runs, wherever it started", () => {
    for (const section of ["decks", "analytics", "home"] as const) {
      const view = renderNavigation({ section, activeWorkflow: "practice" });
      expect(current()).toEqual(["nav-review"]);
      view.unmount();
    }
  });

  it("shows today's count on Review without changing its name", () => {
    renderNavigation({ dueToday: 47 });
    const review = screen.getByRole("button", { name: "Review" });
    expect(within(review).getByText("47")).toBeTruthy();
    expect(document.getElementById(review.getAttribute("aria-describedby") ?? "")?.textContent).toBe(
      "47 cards due today",
    );
  });

  it("shows no badge while the count is unknown or zero", () => {
    for (const dueToday of [null, 0]) {
      const view = renderNavigation({ dueToday });
      expect(view.container.querySelector('[data-slot="nav-badge"]')).toBeNull();
      expect(screen.getByRole("button", { name: "Review" }).hasAttribute("aria-describedby")).toBe(false);
      view.unmount();
    }
  });

  it("caps the badge and keeps the exact count in its description", () => {
    expect(dueTodayBadge(1)).toEqual({ value: "1", description: "1 card due today" });
    expect(dueTodayBadge(1234)).toEqual({ value: "999+", description: "1,234 cards due today" });
  });
});

import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { LayoutModeProvider } from "../layout/LayoutModeProvider";
import { ResponsiveActionGroup, type ResponsiveAction } from "./ResponsiveActionGroup";

const originalMatchMedia = window.matchMedia;
afterEach(() => {
  window.matchMedia = originalMatchMedia;
  delete document.documentElement.dataset.theme;
});

function setLayout(mode: "wide" | "medium" | "compact") {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: query.includes("760") ? mode === "compact" : query.includes("1180") ? mode !== "wide" : false,
    media: query,
    addEventListener: vi.fn(), removeEventListener: vi.fn(), addListener: vi.fn(), removeListener: vi.fn(), dispatchEvent: vi.fn(),
  }));
}

const actions: ResponsiveAction[] = [
  {
    id: "primary",
    label: "Continue",
    description: "Complete the current step before continuing.",
    placement: "always-visible",
    variant: "primary",
    onSelect: vi.fn(),
  },
  { id: "edit", label: "Edit", placement: "wide-and-medium", icon: "edit", presentation: "icon", onSelect: vi.fn() },
  { id: "export", label: "Export", placement: "wide-only", onSelect: vi.fn() },
  { id: "delete", label: "Delete", placement: "overflow-only", destructive: true, onSelect: vi.fn() },
];

function renderGroup(mode: "wide" | "medium" | "compact") {
  setLayout(mode);
  render(<LayoutModeProvider><ResponsiveActionGroup label="Example actions" actions={actions} /></LayoutModeProvider>);
}

describe("ResponsiveActionGroup", () => {
  it("adds supplemental hover and accessible descriptions without changing the label", () => {
    renderGroup("compact");
    const primary = screen.getByRole("button", { name: "Continue" });
    expect(primary.getAttribute("title")).toBe(
      "Complete the current step before continuing.",
    );
    const descriptionId = primary.getAttribute("aria-describedby");
    expect(descriptionId).toBeTruthy();
    expect(document.getElementById(descriptionId as string)?.textContent).toBe(
      "Complete the current step before continuing.",
    );
    expect(primary.textContent).toBe("Continue");
  });

  it.each([
    ["wide", ["Continue", "Edit", "Export"], ["Delete"]],
    ["medium", ["Continue", "Edit"], ["Export", "Delete"]],
    ["compact", ["Continue"], ["Edit", "Export", "Delete"]],
  ] as const)("applies the %s placement contract", (mode, direct, collapsed) => {
    renderGroup(mode);
    for (const label of direct) expect(screen.getByRole("button", { name: label })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    for (const label of collapsed) expect(screen.getByRole("menuitem", { name: label })).toBeTruthy();
  });

  it("rejects unstable duplicate IDs", () => {
    setLayout("wide");
    expect(() => render(<LayoutModeProvider><ResponsiveActionGroup label="Bad" actions={[actions[0], actions[0]]} /></LayoutModeProvider>)).toThrow(
      "ResponsiveActionGroup requires unique, nonblank action IDs.",
    );
  });

  it("rejects a group that could hide every action", () => {
    setLayout("compact");
    expect(() =>
      render(
        <LayoutModeProvider>
          <ResponsiveActionGroup
            label="Bad"
            actions={[{ id: "hidden", label: "Hidden", placement: "wide-only", onSelect: vi.fn() }]}
          />
        </LayoutModeProvider>,
      ),
    ).toThrow("ResponsiveActionGroup requires an always-visible primary action.");
  });

  it.each(["light", "dark", "console", "synthwave"])(
    "keeps the primary action direct in the %s theme",
    (theme) => {
      document.documentElement.dataset.theme = theme;
      renderGroup("compact");
      expect(screen.getByRole("button", { name: "Continue" })).toBeTruthy();
    },
  );
});

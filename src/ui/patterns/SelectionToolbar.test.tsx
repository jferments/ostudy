import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { LayoutModeProvider } from "../layout/LayoutModeProvider";
import { SelectionToolbar } from "./SelectionToolbar";

const originalMatchMedia = window.matchMedia;
afterEach(() => {
  window.matchMedia = originalMatchMedia;
  delete document.documentElement.dataset.theme;
});

function renderToolbar(mode: "wide" | "compact" = "wide") {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: query.includes("760") ? mode === "compact" : query.includes("1180") ? mode === "compact" : false,
    media: query,
    addEventListener: vi.fn(), removeEventListener: vi.fn(), addListener: vi.fn(), removeListener: vi.fn(), dispatchEvent: vi.fn(),
  }));
  const onPractice = vi.fn();
  const onMove = vi.fn();
  const onClear = vi.fn();
  render(
    <LayoutModeProvider>
      <SelectionToolbar
        selectedCount={2}
        primaryAction={{ id: "practice", label: "Practice selected", onSelect: onPractice }}
        secondaryActions={[{ id: "move", label: "Move selected", placement: "wide-and-medium", onSelect: onMove }]}
        onClearSelection={onClear}
      />
    </LayoutModeProvider>,
  );
  return { onClear, onMove, onPractice };
}

describe("SelectionToolbar", () => {
  it("announces the count and exposes direct primary, secondary, and clear actions", () => {
    const callbacks = renderToolbar();
    const toolbar = screen.getByRole("region", { name: "Selection actions" });
    expect(screen.getByText("2 items selected").getAttribute("aria-live")).toBe("polite");
    fireEvent.click(screen.getByRole("button", { name: "Practice selected" }));
    fireEvent.click(screen.getByRole("button", { name: "Move selected" }));
    fireEvent.click(screen.getByRole("button", { name: "Clear selection" }));
    expect(callbacks.onPractice).toHaveBeenCalledOnce();
    expect(callbacks.onMove).toHaveBeenCalledOnce();
    expect(callbacks.onClear).toHaveBeenCalledOnce();
    expect(toolbar.getAttribute("data-selection-toolbar")).toBe("true");
  });

  it("keeps the primary direct and moves responsive secondary actions into compact overflow", () => {
    renderToolbar("compact");
    expect(screen.getByRole("button", { name: "Practice selected" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Move selected" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "More selection actions" }));
    expect(screen.getByRole("menuitem", { name: "Move selected" })).toBeTruthy();
  });

  it("does not render without an active selection", () => {
    render(<SelectionToolbar selectedCount={0} primaryAction={{ id: "practice", label: "Practice", onSelect: vi.fn() }} onClearSelection={vi.fn()} />);
    expect(screen.queryByRole("region", { name: "Selection actions" })).toBeNull();
  });

  it.each(["light", "dark", "console", "synthwave"])(
    "keeps count and primary-action semantics in the %s theme",
    (theme) => {
      document.documentElement.dataset.theme = theme;
      renderToolbar();
      expect(screen.getByText("2 items selected")).toBeTruthy();
      expect(screen.getByRole("button", { name: "Practice selected" })).toBeTruthy();
    },
  );
});

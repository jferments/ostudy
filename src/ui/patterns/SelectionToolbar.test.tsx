import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { LayoutModeProvider } from "../layout/LayoutModeProvider";
import { SelectionToolbar } from "./SelectionToolbar";
import { matchesLayoutQuery, layoutWidths } from "../../test/viewport";

const originalMatchMedia = window.matchMedia;

afterEach(() => {
  window.matchMedia = originalMatchMedia;
});

function setLayout(mode: "wide" | "medium" | "compact") {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: matchesLayoutQuery(query, layoutWidths[mode]),
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

function renderToolbar(mode: "wide" | "medium" | "compact", overlay = false) {
  setLayout(mode);
  const onPrimary = vi.fn();
  const onMove = vi.fn();
  const onClear = vi.fn();
  const view = render(
    <LayoutModeProvider>
      <SelectionToolbar
        overlay={overlay}
        selectedCount={3}
        label="Selected deck actions"
        primaryAction={{ id: "practice", label: "Practice", onSelect: onPrimary }}
        secondaryActions={[
          {
            id: "move",
            label: "Move",
            placement: "wide-only",
            onSelect: onMove,
          },
        ]}
        onClearSelection={onClear}
      />
    </LayoutModeProvider>,
  );
  return { ...view, onClear, onMove, onPrimary };
}

describe("SelectionToolbar", () => {
  it("announces the count and keeps the primary action direct", () => {
    const { onPrimary } = renderToolbar("wide");
    const toolbar = screen.getByRole("region", { name: "Selected deck actions" });
    expect(within(toolbar).getByText("3 items selected")).toBeTruthy();
    fireEvent.click(within(toolbar).getByRole("button", { name: "Practice" }));
    expect(onPrimary).toHaveBeenCalledOnce();
  });

  it("promotes a sole compact secondary action instead of creating an ellipsis menu", () => {
    const { onMove } = renderToolbar("compact");
    const toolbar = screen.getByRole("region", { name: "Selected deck actions" });
    const move = within(toolbar).getByRole("button", { name: "Move" });
    expect(move).toBeTruthy();
    expect(within(toolbar).queryByRole("button", { name: "More selection actions" })).toBeNull();
    fireEvent.click(move);
    expect(onMove).toHaveBeenCalledOnce();
  });

  it("clears through the direct icon action", () => {
    const { onClear } = renderToolbar("wide");
    fireEvent.click(screen.getByRole("button", { name: "Clear selection" }));
    expect(onClear).toHaveBeenCalledOnce();
  });

  it("can render in a zero-height overlay layer without reserving content space", () => {
    const { container } = renderToolbar("wide", true);
    const layer = container.querySelector('[data-selection-toolbar-layer="true"]') as HTMLElement;
    expect(layer).toBeTruthy();
    expect(layer.className).toContain("tw:h-0");
    expect(layer.className).toContain("tw:overflow-visible");
    // The overlay floats on the token layer scale, above other sticky content.
    expect(screen.getByRole("region", { name: "Selected deck actions" }).className).toMatch(/tw:z-\S*--ostudy-layer-sticky\)\+1/u);
  });

  it("renders nothing when no items are selected", () => {
    setLayout("wide");
    const { container } = render(
      <LayoutModeProvider>
        <SelectionToolbar
          selectedCount={0}
          primaryAction={{ id: "practice", label: "Practice", onSelect: vi.fn() }}
          onClearSelection={vi.fn()}
        />
      </LayoutModeProvider>,
    );
    expect(container.childElementCount).toBe(0);
  });
});

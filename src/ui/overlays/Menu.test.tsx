import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { OverlayProvider } from "./OverlayProvider";
import { Menu } from "./Menu";

const originalMatchMedia = window.matchMedia;

afterEach(() => {
  window.matchMedia = originalMatchMedia;
});

function renderMenu(onSecond = vi.fn()) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
  render(
    <OverlayProvider>
      <Menu
        label="Actions"
        items={[
          { label: "First action", onSelect: vi.fn() },
          { label: "Second action", onSelect: onSecond },
          { label: "Delete", destructive: true, onSelect: vi.fn() },
        ]}
      />
    </OverlayProvider>,
  );
  return onSecond;
}

describe("Menu", () => {
  it("moves focus through menu items and restores it on Escape", async () => {
    renderMenu();
    const trigger = screen.getByRole("button", { name: "Actions" });
    trigger.focus();
    fireEvent.click(trigger);

    const first = await screen.findByRole("menuitem", { name: "First action" });
    const second = screen.getByRole("menuitem", { name: "Second action" });
    await waitFor(() => expect(document.activeElement).toBe(first));

    fireEvent.keyDown(screen.getByRole("menu", { name: "Actions" }), { key: "ArrowDown" });
    expect(document.activeElement).toBe(second);
    fireEvent.keyDown(screen.getByRole("menu", { name: "Actions" }), { key: "Escape" });
    await waitFor(() => expect(document.activeElement).toBe(trigger));
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("closes before running the selected command", async () => {
    const onSecond = renderMenu();
    fireEvent.click(screen.getByRole("button", { name: "Actions" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Second action" }));

    expect(onSecond).toHaveBeenCalledOnce();
    expect(screen.queryByRole("menu")).toBeNull();
  });
});

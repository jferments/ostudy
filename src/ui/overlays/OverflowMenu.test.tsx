import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { LayoutModeProvider } from "../layout/LayoutModeProvider";
import { OverlayProvider } from "./OverlayProvider";
import { OverflowMenu } from "./OverflowMenu";

const originalMatchMedia = window.matchMedia;

afterEach(() => {
  window.matchMedia = originalMatchMedia;
  delete document.documentElement.dataset.theme;
});

function setLayout(mode: "wide" | "medium" | "compact") {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches:
      query === "(max-width: 760px)"
        ? mode === "compact"
        : query === "(max-width: 1180px)"
          ? mode !== "wide"
          : false,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

function renderMenu(mode: "wide" | "medium" | "compact" = "wide") {
  setLayout(mode);
  const onFirst = vi.fn();
  const onDelete = vi.fn();
  render(
    <LayoutModeProvider>
      <OverlayProvider>
        <OverflowMenu
          label="Card actions"
          items={[
            { id: "first", label: "First", onSelect: onFirst },
            { id: "disabled", label: "Disabled", disabled: true, onSelect: vi.fn() },
            { id: "last", label: "Last", onSelect: vi.fn() },
            { id: "delete", label: "Delete", destructive: true, onSelect: onDelete },
          ]}
        />
      </OverlayProvider>
    </LayoutModeProvider>,
  );
  return { onDelete, onFirst };
}

describe("OverflowMenu", () => {
  it("uses roving focus, skips disabled items, activates with Space, and restores on Escape", async () => {
    const { onFirst } = renderMenu();
    const trigger = screen.getByRole("button", { name: "Card actions" });
    trigger.focus();
    fireEvent.click(trigger);
    const menu = await screen.findByRole("menu", { name: "Card actions" });
    const first = within(menu).getByRole("menuitem", { name: "First" });
    const last = within(menu).getByRole("menuitem", { name: "Last" });
    await waitFor(() => expect(document.activeElement).toBe(first));
    expect(first.getAttribute("tabindex")).toBe("0");

    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(document.activeElement).toBe(last);
    expect((within(menu).getByRole("menuitem", { name: "Disabled" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.keyDown(menu, { key: "Home" });
    fireEvent.keyDown(menu, { key: " " });
    expect(onFirst).toHaveBeenCalledOnce();
    expect(screen.queryByRole("menu")).toBeNull();

    fireEvent.click(trigger);
    fireEvent.keyDown(await screen.findByRole("menu"), { key: "Escape" });
    await waitFor(() => expect(document.activeElement).toBe(trigger));
  });

  it("dismisses on an outside pointer and keeps destructive actions in a separated final group", async () => {
    renderMenu();
    const trigger = screen.getByRole("button", { name: "Card actions" });
    fireEvent.click(trigger);
    const menu = await screen.findByRole("menu");
    const separator = within(menu).getByRole("separator");
    expect(separator.nextElementSibling?.textContent).toBe("Delete");

    fireEvent.pointerDown(document.body);
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(document.activeElement).toBe(trigger);
  });

  it("closes and restores the trigger before invoking a selected callback", async () => {
    renderMenu();
    const trigger = screen.getByRole("button", { name: "Card actions" });
    const observed: Array<{ open: boolean; focused: boolean }> = [];
    render(
      <OverflowMenu
        label="Ordering actions"
        items={[
          {
            id: "observe",
            label: "Observe",
            onSelect: () => {
              observed.push({
                open: screen.queryByRole("menu", { name: "Ordering actions" }) !== null,
                focused: document.activeElement === screen.getByRole("button", { name: "Ordering actions" }),
              });
            },
          },
        ]}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Ordering actions" }));
    fireEvent.keyDown(await screen.findByRole("menu", { name: "Ordering actions" }), {
      key: "Enter",
    });
    expect(observed).toEqual([{ open: false, focused: true }]);
    expect(trigger).toBeTruthy();
  });

  it("uses the shared compact Sheet lifecycle", async () => {
    renderMenu("compact");
    const trigger = screen.getByRole("button", { name: "Card actions" });
    fireEvent.click(trigger);
    const dialog = await screen.findByRole("dialog", { name: "Card actions" });
    expect(dialog.getAttribute("data-presentation")).toBe("sheet");
    expect(within(dialog).getByRole("menu", { name: "Card actions" })).toBeTruthy();
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(document.activeElement).toBe(trigger);
  });

  it.each(["light", "dark", "console", "synthwave"])(
    "keeps destructive menu semantics in the %s theme",
    (theme) => {
      document.documentElement.dataset.theme = theme;
      renderMenu();
      fireEvent.click(screen.getByRole("button", { name: "Card actions" }));
      expect(screen.getByRole("menuitem", { name: "Delete" }).className).toBeTruthy();
    },
  );
});

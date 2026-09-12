import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LayoutModeProvider } from "../layout/LayoutModeProvider";
import { OverlayProvider } from "./OverlayProvider";
import { OverflowMenu } from "./OverflowMenu";
const originalMatchMedia = window.matchMedia;
afterEach(() => { window.matchMedia = originalMatchMedia; delete document.documentElement.dataset.theme; });
function renderMenu(compact = false) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({ matches: compact && (query === "(max-width: 760px)" || query === "(max-width: 1180px)"), media: query, onchange: null, addEventListener: vi.fn(), removeEventListener: vi.fn(), addListener: vi.fn(), removeListener: vi.fn(), dispatchEvent: vi.fn() }));
  const onFirst = vi.fn();
  render(<LayoutModeProvider><OverlayProvider><OverflowMenu label="Card actions" items={[
    { id: "first", label: "First", onSelect: onFirst }, { id: "disabled", label: "Disabled", disabled: true, onSelect: vi.fn() }, { id: "last", label: "Last", onSelect: vi.fn() }, { id: "delete", label: "Delete", destructive: true, onSelect: vi.fn() },
  ]} /></OverlayProvider></LayoutModeProvider>);
  return onFirst;
}
describe("OverflowMenu", () => {
  it("uses library roving focus, makes disabled items discoverable but inactive, and restores the trigger", async () => {
    const onFirst = renderMenu(); const trigger = screen.getByRole("button", { name: "Card actions" });
    trigger.focus(); fireEvent.click(trigger);
    const menu = await screen.findByRole("menu", { name: "Card actions" });
    const first = within(menu).getByRole("menuitem", { name: "First" });
    const last = within(menu).getByRole("menuitem", { name: "Last" });
    await waitFor(() => expect(document.activeElement).toBe(first));
    fireEvent.keyDown(first, { key: "ArrowDown" });
    const disabled = within(menu).getByRole("menuitem", { name: "Disabled" });
    await waitFor(() => expect(document.activeElement).toBe(disabled));
    fireEvent.click(disabled);
    expect(screen.getByRole("menu")).toBe(menu);
    expect(onFirst).not.toHaveBeenCalled();
    fireEvent.keyDown(disabled, { key: "ArrowDown" });
    await waitFor(() => expect(document.activeElement).toBe(last));
    expect(within(menu).getByRole("menuitem", { name: "Disabled" }).getAttribute("aria-disabled")).toBe("true");
    fireEvent.keyDown(last, { key: "Home" }); await waitFor(() => expect(document.activeElement).toBe(first));
    fireEvent.keyDown(first, { key: " " }); fireEvent.keyUp(first, { key: " " });
    expect(onFirst).toHaveBeenCalledOnce(); await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    fireEvent.click(trigger); fireEvent.keyDown(await screen.findByRole("menu"), { key: "Escape" });
    await waitFor(() => expect(document.activeElement).toBe(trigger));
  });
  it("dismisses outside and separates destructive actions", async () => {
    renderMenu(); const trigger = screen.getByRole("button", { name: "Card actions" }); trigger.focus(); fireEvent.click(trigger);
    const menu = await screen.findByRole("menu");
    expect(within(menu).getByRole("separator").nextElementSibling?.textContent).toBe("Delete");
    fireEvent.mouseDown(document.body); fireEvent.click(document.body);
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  });
  it("renders a single command directly", () => {
    const onSelect = vi.fn(); render(<OverflowMenu label="Ordering actions" items={[{ id: "observe", label: "Observe", onSelect }]} />);
    expect(screen.queryByRole("button", { name: "Ordering actions" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Observe" })); expect(onSelect).toHaveBeenCalledOnce(); expect(screen.queryByRole("menu")).toBeNull();
  });
  it("uses the shared compact Sheet lifecycle and touch actions", async () => {
    renderMenu(true); const trigger = screen.getByRole("button", { name: "Card actions" }); trigger.focus(); fireEvent.click(trigger);
    const dialog = await screen.findByRole("dialog", { name: "Card actions" });
    expect(dialog.getAttribute("data-presentation")).toBe("sheet");
    expect(within(dialog).getByRole("button", { name: "First" })).toBeTruthy();
    expect((within(dialog).getByRole("button", { name: "Disabled" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.keyDown(dialog, { key: "Escape" }); await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(trigger));
  });
  it.each(["light", "dark", "console", "synthwave"])("keeps destructive actions in %s", async (theme) => {
    document.documentElement.dataset.theme = theme; renderMenu(); fireEvent.click(screen.getByRole("button", { name: "Card actions" }));
    expect((await screen.findByRole("menuitem", { name: "Delete" })).className).toContain("tw:text-destructive");
  });
});

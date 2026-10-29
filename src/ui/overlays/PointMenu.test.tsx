import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LayoutModeProvider } from "../layout/LayoutModeProvider";
import { OverlayProvider } from "./OverlayProvider";
import { PointMenu, type PointMenuItem } from "./PointMenu";
import { matchesLayoutQuery, layoutWidths } from "../../test/viewport";

const originalMatchMedia = window.matchMedia;
afterEach(() => { window.matchMedia = originalMatchMedia; });

function Opened({ items }: { items: PointMenuItem[] }) {
  const [point, setPoint] = useState<{ x: number; y: number } | null>({ x: 200, y: 120 });
  return <PointMenu label="Topic" point={point} items={items} onClose={() => setPoint(null)} />;
}

function renderMenu(compact: boolean) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({ matches: matchesLayoutQuery(query, layoutWidths[compact ? "compact" : "wide"]), media: query, onchange: null, addEventListener: vi.fn(), removeEventListener: vi.fn(), addListener: vi.fn(), removeListener: vi.fn(), dispatchEvent: vi.fn() }));
  const onEdit = vi.fn();
  const onRed = vi.fn();
  const onDelete = vi.fn();
  render(<LayoutModeProvider><OverlayProvider><Opened items={[
    { id: "edit", label: "Edit", onSelect: onEdit },
    { id: "color", label: "Colour", items: [
      { id: "blue", label: "Blue", checked: true, onSelect: vi.fn() },
      { id: "red", label: "Red", checked: false, onSelect: onRed },
    ] },
    { id: "delete", label: "Delete", destructive: true, onSelect: onDelete },
  ]} /></OverlayProvider></LayoutModeProvider>);
  return { onEdit, onRed, onDelete };
}

describe("PointMenu", () => {
  it("opens where it was asked for, runs an action and closes", async () => {
    const { onEdit } = renderMenu(false);
    const menu = await screen.findByRole("menu", { name: "Topic" });
    expect(within(menu).getByRole("menuitem", { name: "Colour" })).toBeTruthy();
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Edit" }));
    expect(onEdit).toHaveBeenCalledOnce();
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  });

  it("shows a group of choices as a section of a phone's sheet", async () => {
    const { onRed } = renderMenu(true);
    const sheet = await screen.findByRole("dialog", { name: "Topic" });
    const colours = within(sheet).getByRole("region", { name: "Colour" });
    expect(within(colours).getByRole("button", { name: "Blue" }).getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(within(colours).getByRole("button", { name: "Red" }));
    expect(onRed).toHaveBeenCalledOnce();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
});

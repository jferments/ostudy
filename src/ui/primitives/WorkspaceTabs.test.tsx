import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import { WorkspaceTabs } from "./Tabs";

function Harness() {
  const [value, setValue] = useState<"decks" | "search">("decks");
  return <WorkspaceTabs
    value={value}
    onValueChange={setValue}
    ariaLabel="Flashcards workspaces"
    tabs={[
      { value: "decks", label: "Decks", compactLabel: "Decks", icon: <svg aria-hidden="true" /> },
      { value: "search", label: "Search/Filter", compactLabel: "Search", icon: <svg aria-hidden="true" /> },
    ]}
  ><p>{value === "decks" ? "Deck library" : "Card search"}</p></WorkspaceTabs>;
}

describe("responsive workspace tab names", () => {
  it("keeps full accessible names independent of compact text and decorative icons", () => {
    render(<Harness />);
    const list = screen.getByRole("tablist", { name: "Flashcards workspaces" });
    expect(within(list).getAllByRole("tab").map((tab) => tab.getAttribute("aria-label")))
      .toEqual(["Decks", "Search/Filter"]);
    const search = within(list).getByRole("tab", { name: "Search/Filter", exact: true });
    expect(within(list).queryByRole("tab", { name: "Search", exact: true })).toBeNull();
    fireEvent.click(search);
    expect(search.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tabpanel").textContent).toContain("Card search");
  });

  it("preserves roving keyboard focus and panel selection with responsive labels", async () => {
    render(<Harness />);
    const decks = screen.getByRole("tab", { name: "Decks", exact: true });
    const search = screen.getByRole("tab", { name: "Search/Filter", exact: true });
    act(() => decks.focus());
    fireEvent.keyDown(decks, { key: "ArrowRight" });
    await waitFor(() => {
      expect(document.activeElement).toBe(search);
      expect(search.getAttribute("aria-selected")).toBe("true");
      expect(screen.getByRole("tabpanel").textContent).toContain("Card search");
    });
    fireEvent.keyDown(search, { key: "Home" });
    await waitFor(() => {
      expect(document.activeElement).toBe(decks);
      expect(decks.getAttribute("aria-selected")).toBe("true");
      expect(screen.getByRole("tabpanel").textContent).toContain("Deck library");
    });
  });
});

import { readFileSync } from "node:fs";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Button } from "./Button";
import { ListRow, ListRowMedia, ListRows } from "./ListRow";

const bridge = readFileSync("src/ui/tokens/generated/tailwind-theme.css", "utf8");

/** Every font size an element and its descendants set, as the type-scale tokens they resolve to. */
function fontSizeTokens(root: Element): string[] {
  const tokens: string[] = [];
  for (const element of [root, ...root.querySelectorAll("*")]) {
    for (const name of (element.getAttribute("class") ?? "").split(/\s+/u)) {
      const step = /^tw:(?:[^:]+:)*text-(.+)$/u.exec(name)?.[1];
      if (!step) continue;
      if (/^\[\d/u.test(step)) throw new Error(`${name} is off the type scale`);
      const token = new RegExp(`--text-${step}: var\\((--ostudy-[a-z0-9-]+)\\);`, "u").exec(bridge)?.[1];
      if (token) tokens.push(token);
    }
  }
  return tokens;
}

function sourceRow(props: Partial<Parameters<typeof ListRow>[0]> = {}) {
  return (
    <ListRows>
      <ListRow
        leading={<ListRowMedia label="PDF" />}
        title="Renal physiology"
        meta={<><span>Pages 4 to 19</span><span>Added 3 May</span></>}
        trailing={<Button size="small">Remove</Button>}
        {...props}
      />
    </ListRows>
  );
}

describe("ListRow", () => {
  it("reads as a list item: title, meta lines, then the trailing actions", () => {
    render(sourceRow());
    const row = within(screen.getByRole("list")).getByRole("listitem");
    const title = within(row).getByText("Renal physiology");
    const range = within(row).getByText("Pages 4 to 19");
    const added = within(row).getByText("Added 3 May");
    const remove = within(row).getByRole("button", { name: "Remove" });

    expect(title.tagName).toBe("STRONG");
    for (const [earlier, later] of [[title, range], [range, added], [added, remove]]) {
      expect(earlier!.compareDocumentPosition(later!) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
    }
    expect(row.getAttribute("data-density")).toBe("comfortable");
    expect(within(row).queryByRole("button", { name: /Renal physiology/u })).toBeNull();
  });

  it("keeps the media tile out of the accessible text and draws its label on the type scale", () => {
    render(sourceRow());
    const tile = screen.getByText("PDF");
    expect(tile.closest("[aria-hidden='true']")).toBe(tile);
    expect(fontSizeTokens(tile)).toEqual(["--ostudy-font-size-caption"]);
  });

  it("makes the tile and text one button that opens the row, with the row's actions outside it", () => {
    const onActivate = vi.fn();
    render(sourceRow({ onActivate, chevron: true }));
    const open = screen.getByRole("button", { name: /^Renal physiology/u });
    const remove = screen.getByRole("button", { name: "Remove" });

    expect(open.contains(screen.getByText("PDF"))).toBe(true);
    expect(open.contains(remove)).toBe(false);
    expect(open.querySelector("svg[data-icon='chevron-right']")?.getAttribute("aria-hidden")).toBe("true");
    fireEvent.click(open);
    expect(onActivate).toHaveBeenCalledTimes(1);
    fireEvent.click(remove);
    expect(onActivate).toHaveBeenCalledTimes(1);
  });

  it("can disable opening the row without disabling its actions", () => {
    const onActivate = vi.fn();
    render(sourceRow({ onActivate, activateDisabled: true }));
    expect((screen.getByRole("button", { name: /^Renal physiology/u }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Remove" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("sets dense titles a step below comfortable ones on the type scale", () => {
    render(
      <ListRows>
        <ListRow title="Comfortable" meta="Two sources" />
        <ListRow density="dense" title="Dense" meta="Edited today" />
      </ListRows>,
    );
    const [comfortable, dense] = screen.getAllByRole("listitem");
    expect(dense?.getAttribute("data-density")).toBe("dense");
    expect(fontSizeTokens(within(comfortable!).getByText("Comfortable"))).toEqual(["--ostudy-font-size-body"]);
    expect(fontSizeTokens(within(dense!).getByText("Dense"))).toEqual(["--ostudy-font-size-small"]);
    for (const row of [comfortable!, dense!]) expect(() => fontSizeTokens(row)).not.toThrow();
  });

  it("holds article rows in a plain container and marks the selected one", () => {
    render(
      <ListRows as="div">
        <ListRow as="article" aria-label="First session" title="Cardiology" />
        <ListRow as="article" aria-label="Second session" title="Renal" selected />
      </ListRows>,
    );
    expect(screen.queryByRole("list")).toBeNull();
    expect(screen.getByRole("article", { name: "First session" }).hasAttribute("data-selected")).toBe(false);
    expect(screen.getByRole("article", { name: "Second session" }).getAttribute("data-selected")).toBe("true");
  });
});

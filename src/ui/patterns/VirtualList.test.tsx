import { render, screen, within } from "@testing-library/react";
import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";

import { VirtualList, type VirtualListHandle } from "./VirtualList";

const rows = Array.from({ length: 5000 }, (_, index) => ({ id: `card-${index}`, title: `Card ${index + 1}` }));

function renderList(props: Partial<Parameters<typeof VirtualList<(typeof rows)[number]>>[0]> = {}) {
  return render(
    <div style={{ height: 400 }}>
      <VirtualList
        items={rows}
        itemKey={(row) => row.id}
        renderItem={(row) => <button type="button">{row.title}</button>}
        estimateItemSize={40}
        label="Cards"
        {...props}
      />
    </div>,
  );
}

describe("VirtualList", () => {
  it("renders a viewport's worth of rows from a long list, with list semantics", () => {
    renderList();
    const list = screen.getByRole("list", { name: "Cards" });
    const items = within(list).getAllByRole("listitem");
    expect(items.length).toBeGreaterThan(10);
    expect(items.length).toBeLessThan(100);
    expect(items[0]!.getAttribute("aria-setsize")).toBe("5000");
    expect(items[0]!.getAttribute("aria-posinset")).toBe("1");
    expect(screen.getByRole("button", { name: "Card 1" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Card 5000" })).toBeNull();
  });

  it("asks for the next page only when the end of the list is rendered", () => {
    const onEndReached = vi.fn();
    const view = renderList({ onEndReached });
    expect(onEndReached).not.toHaveBeenCalled();
    view.rerender(
      <VirtualList
        items={rows.slice(0, 12)}
        itemKey={(row) => row.id}
        renderItem={(row) => <span>{row.title}</span>}
        estimateItemSize={40}
        label="Cards"
        onEndReached={onEndReached}
      />,
    );
    expect(onEndReached).toHaveBeenCalledTimes(1);
  });

  it("renders a footer inside the scroll area and nothing else for an empty list", () => {
    renderList({ items: [], footer: <p>Loading more cards</p> });
    expect(within(screen.getByRole("list", { name: "Cards" })).queryAllByRole("listitem")).toHaveLength(0);
    expect(screen.getByText("Loading more cards")).toBeTruthy();
  });

  it("exposes scrollToIndex", () => {
    const ref = createRef<VirtualListHandle>();
    renderList({ ref });
    expect(() => ref.current?.scrollToIndex(4000)).not.toThrow();
  });
});

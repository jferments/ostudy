import { fireEvent, render, screen, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { LibrarySection } from "../../navigation/appDestinations";
import { matchesLayoutQuery } from "../../test/viewport";
import { LayoutModeProvider } from "../../ui/layout/LayoutModeProvider";
import { LibraryFrame, useLibraryFullPage } from "./LibraryFrame";

const media = window.matchMedia;
afterEach(() => {
  window.matchMedia = media;
  delete document.documentElement.dataset.layoutMode;
});

function setViewport(width: number): void {
  window.matchMedia = vi.fn(
    (query: string) =>
      ({
        matches: matchesLayoutQuery(query, width),
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      }) as unknown as MediaQueryList,
  );
}

function renderLibrary(view: LibrarySection, width = 1440) {
  setViewport(width);
  const onSelectView = vi.fn<(view: LibrarySection) => void>();
  const result = render(
    <LayoutModeProvider>
      <LibraryFrame view={view} onSelectView={onSelectView}>
        <h1>{`Hosted ${view}`}</h1>
      </LibraryFrame>
    </LayoutModeProvider>,
  );
  return { ...result, onSelectView };
}

function views() {
  return within(screen.getByRole("navigation", { name: "Library" })).getAllByRole("button");
}

describe("LibraryFrame", () => {
  it.each([390, 1000, 1440])("names the three views in order at %s px", (width) => {
    renderLibrary("decks", width);
    expect(views().map((button) => button.textContent)).toEqual(["Workbench", "Documents", "Study guides"]);
  });

  it.each([
    ["decks", "Workbench"],
    ["documents", "Documents"],
    ["study-guides", "Study guides"],
  ] as const)("marks %s as the current view and hosts it below the switcher", (view, label) => {
    renderLibrary(view);
    const current = views().filter((button) => button.getAttribute("aria-current") === "page");
    expect(current.map((button) => button.textContent)).toEqual([label]);
    expect(screen.getByRole("heading", { name: `Hosted ${view}` })).toBeTruthy();
  });

  it("reports every choice, including the open view, so it can return to its first page", () => {
    const { onSelectView } = renderLibrary("study-guides", 390);
    fireEvent.click(screen.getByRole("button", { name: "Documents" }));
    fireEvent.click(screen.getByRole("button", { name: "Study guides" }));
    fireEvent.click(screen.getByRole("button", { name: "Workbench" }));
    expect(onSelectView.mock.calls.map(([view]) => view)).toEqual(["documents", "study-guides", "decks"]);
  });

  it("keeps the switcher in place while the hosted view changes", () => {
    const { rerender, onSelectView } = renderLibrary("decks");
    const switcher = screen.getByRole("navigation", { name: "Library" });
    rerender(
      <LayoutModeProvider>
        <LibraryFrame view="documents" onSelectView={onSelectView}>
          <h1>Hosted documents</h1>
        </LibraryFrame>
      </LayoutModeProvider>,
    );
    expect(screen.getByRole("navigation", { name: "Library" })).toBe(switcher);
    expect(screen.getByRole("heading", { name: "Hosted documents" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Hosted decks" })).toBeNull();
  });

  it("steps aside while a hosted view shows a page of its own", () => {
    function Guides({ open }: { open: boolean }) {
      useLibraryFullPage(open);
      return <h1>{open ? "Open guide" : "Study Guides"}</h1>;
    }
    setViewport(1440);
    const frame = (content: ReactNode) => (
      <LayoutModeProvider>
        <LibraryFrame view="study-guides" onSelectView={vi.fn()}>
          {content}
        </LibraryFrame>
      </LayoutModeProvider>
    );
    const { rerender } = render(frame(<Guides open />));
    expect(screen.queryByRole("navigation", { name: "Library" })).toBeNull();
    // Without the frame's inset, the page's own sticky top bar reaches the edges.
    expect(screen.getByTestId("library").className).toContain("tw:p-0");
    const heading = screen.getByRole("heading", { name: "Open guide" });

    rerender(frame(<Guides open={false} />));
    expect(screen.getByRole("navigation", { name: "Library" })).toBeTruthy();
    expect(screen.getByTestId("library").className).toContain("tw:px-page");
    // The view stays mounted while the switcher comes and goes.
    expect(screen.getByRole("heading", { name: "Study Guides" })).toBe(heading);

    // A view that leaves with its page open brings the switcher back too.
    rerender(frame(<Guides open />));
    expect(screen.queryByRole("navigation", { name: "Library" })).toBeNull();
    rerender(frame(<h1>Hosted decks</h1>));
    expect(screen.getByRole("navigation", { name: "Library" })).toBeTruthy();
  });
});

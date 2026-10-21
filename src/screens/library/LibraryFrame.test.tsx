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
      <LibraryFrame view={view} onSelectView={onSelectView} {...SUBJECT_PROPS}>
        <h1>{`Hosted ${view}`}</h1>
      </LibraryFrame>
    </LayoutModeProvider>,
  );
  return { ...result, onSelectView };
}

const SUBJECT_PROPS = { subject: { kind: "all" }, onChooseSubject: vi.fn(), onStartReview: vi.fn(), onPracticeSubject: vi.fn(), showToast: vi.fn() } as const;

function views() {
  return within(screen.getByRole("navigation", { name: "Library pages" })).getAllByRole("button");
}

function names(buttons: HTMLElement[]) {
  return buttons.map((button) => button.getAttribute("aria-label"));
}

describe("LibraryFrame", () => {
  it.each([390, 1000, 1440])("names the six pages in order at %s px", (width) => {
    renderLibrary("decks", width);
    expect(names(views())).toEqual(["Overview", "Lessons", "Study guides", "Documents", "Notes", "Flashcards"]);
  });

  it("lists the subjects beside the page on wide screens and in a sheet on narrower ones", () => {
    renderLibrary("overview", 1440);
    expect(screen.getByRole("complementary", { name: "Library subjects" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2, name: "Everything" })).toBeTruthy();
    expect(screen.queryByTestId("library-switch-subject")).toBeNull();
  });

  it.each([390, 1000])("chooses the subject from the header at %s px", (width) => {
    const { onChooseSubject } = SUBJECT_PROPS;
    renderLibrary("overview", width);
    expect(screen.queryByRole("complementary", { name: "Library subjects" })).toBeNull();
    fireEvent.click(screen.getByTestId("library-switch-subject"));
    const sheet = screen.getByRole("dialog", { name: "Subjects" });
    fireEvent.click(within(sheet).getByTestId("library-subject-all"));
    expect(onChooseSubject).toHaveBeenCalledWith({ kind: "all" });
    expect(screen.queryByRole("dialog", { name: "Subjects" })).toBeNull();
  });

  it.each([
    ["overview", "Overview"],
    ["lessons", "Lessons"],
    ["decks", "Flashcards"],
    ["documents", "Documents"],
    ["study-guides", "Study guides"],
  ] as const)("marks %s as the current view and hosts it below the tabs", (view, label) => {
    renderLibrary(view);
    const current = views().filter((button) => button.getAttribute("aria-current") === "page");
    expect(names(current)).toEqual([label]);
    expect(screen.getByRole("heading", { name: `Hosted ${view}` })).toBeTruthy();
  });

  it("reports every choice, including the open view, so it can return to its first page", () => {
    const { onSelectView } = renderLibrary("study-guides", 390);
    fireEvent.click(screen.getByRole("button", { name: "Documents" }));
    fireEvent.click(screen.getByRole("button", { name: "Study guides" }));
    fireEvent.click(screen.getByRole("button", { name: "Flashcards" }));
    expect(onSelectView.mock.calls.map(([view]) => view)).toEqual(["documents", "study-guides", "decks"]);
  });

  it("keeps the tabs in place while the hosted view changes", () => {
    const { rerender, onSelectView } = renderLibrary("decks");
    const tabs = screen.getByRole("navigation", { name: "Library pages" });
    rerender(
      <LayoutModeProvider>
        <LibraryFrame view="documents" onSelectView={onSelectView} {...SUBJECT_PROPS}>
          <h1>Hosted documents</h1>
        </LibraryFrame>
      </LayoutModeProvider>,
    );
    expect(screen.getByRole("navigation", { name: "Library pages" })).toBe(tabs);
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
        <LibraryFrame view="study-guides" onSelectView={vi.fn()} {...SUBJECT_PROPS}>
          {content}
        </LibraryFrame>
      </LayoutModeProvider>
    );
    const { rerender } = render(frame(<Guides open />));
    expect(screen.queryByRole("navigation", { name: "Library pages" })).toBeNull();
    // Without the frame's inset, the page's own sticky top bar reaches the edges.
    expect(screen.getByTestId("library").className).toContain("tw:p-0");
    const heading = screen.getByRole("heading", { name: "Open guide" });

    rerender(frame(<Guides open={false} />));
    expect(screen.getByRole("navigation", { name: "Library pages" })).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Library pages" }).closest("div.tw\\:px-5")).toBeTruthy();
    // The view stays mounted while the tabs come and go.
    expect(screen.getByRole("heading", { name: "Study Guides" })).toBe(heading);

    // A view that leaves with its page open brings the tabs back too.
    rerender(frame(<Guides open />));
    expect(screen.queryByRole("navigation", { name: "Library pages" })).toBeNull();
    rerender(frame(<h1>Hosted decks</h1>));
    expect(screen.getByRole("navigation", { name: "Library pages" })).toBeTruthy();
  });
});

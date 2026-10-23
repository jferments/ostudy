import { fireEvent, render, screen, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { LibrarySubjectScope, SubjectSection } from "../../navigation/appDestinations";
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

function renderLibrary(view: SubjectSection, width = 1440, subject: LibrarySubjectScope = SUBJECT) {
  setViewport(width);
  const onSelectView = vi.fn<(view: SubjectSection) => void>();
  const result = render(
    <LayoutModeProvider>
      <LibraryFrame view={view} onSelectView={onSelectView} {...SUBJECT_PROPS} subject={subject}>
        <h1>{`Hosted ${view}`}</h1>
      </LibraryFrame>
    </LayoutModeProvider>,
  );
  return { ...result, onSelectView };
}

const SUBJECT: LibrarySubjectScope = { kind: "none" };
const SUBJECT_PROPS = {
  subject: SUBJECT,
  onChooseSubject: vi.fn(),
  onOpenLibrary: vi.fn(),
  onStartReview: vi.fn(),
  onPracticeSubject: vi.fn(),
  showToast: vi.fn(),
} as const;

function views() {
  return within(screen.getByRole("navigation", { name: "Subject pages" })).getAllByRole("button");
}

function names(buttons: HTMLElement[]) {
  return buttons.map((button) => button.getAttribute("aria-label"));
}

describe("LibraryFrame", () => {
  it.each([390, 1000, 1440])("names the subject's five pages in order at %s px", (width) => {
    renderLibrary("decks", width);
    expect(names(views())).toEqual(["Flashcards", "Study guides", "Lessons", "Podcasts", "Documents"]);
  });

  it("leads back to Library and has no list of subjects beside the page", () => {
    renderLibrary("documents", 1440);
    expect(screen.queryByRole("complementary", { name: "Library subjects" })).toBeNull();
    expect(screen.getByRole("heading", { level: 2, name: "No subject" })).toBeTruthy();
    fireEvent.click(screen.getByTestId("library-back"));
    expect(SUBJECT_PROPS.onOpenLibrary).toHaveBeenCalled();
  });

  it("shows All cards without a subject's tabs", () => {
    renderLibrary("decks", 1440, { kind: "all" });
    expect(screen.getByRole("heading", { level: 2, name: "All cards" })).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "Subject pages" })).toBeNull();
    expect(screen.getByRole("heading", { name: "Hosted decks" })).toBeTruthy();
  });

  it.each([
    ["lessons", "Lessons"],
    ["podcasts", "Podcasts"],
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
    const tabs = screen.getByRole("navigation", { name: "Subject pages" });
    rerender(
      <LayoutModeProvider>
        <LibraryFrame view="documents" onSelectView={onSelectView} {...SUBJECT_PROPS}>
          <h1>Hosted documents</h1>
        </LibraryFrame>
      </LayoutModeProvider>,
    );
    expect(screen.getByRole("navigation", { name: "Subject pages" })).toBe(tabs);
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
    expect(screen.queryByRole("navigation", { name: "Subject pages" })).toBeNull();
    // Without the frame's inset, the page's own sticky top bar reaches the edges.
    expect(screen.getByTestId("library").className).toContain("tw:p-0");
    const heading = screen.getByRole("heading", { name: "Open guide" });

    rerender(frame(<Guides open={false} />));
    expect(screen.getByRole("navigation", { name: "Subject pages" })).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Subject pages" }).closest("div.tw\\:px-5")).toBeTruthy();
    // The view stays mounted while the tabs come and go.
    expect(screen.getByRole("heading", { name: "Study Guides" })).toBe(heading);

    // A view that leaves with its page open brings the tabs back too.
    rerender(frame(<Guides open />));
    expect(screen.queryByRole("navigation", { name: "Subject pages" })).toBeNull();
    rerender(frame(<h1>Hosted decks</h1>));
    expect(screen.getByRole("navigation", { name: "Subject pages" })).toBeTruthy();
  });
});

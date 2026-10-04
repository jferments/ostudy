import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { PageHeader } from "../patterns/WorkspacePatterns";
import { Button } from "./Button";
import { Panel } from "./Panel";
import { SectionHeading } from "./SectionHeading";

function following(first: Element, second: Element): boolean {
  return (first.compareDocumentPosition(second) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
}

describe("headings", () => {
  it("gives a page one h1, with its description and actions after the title", () => {
    render(<PageHeader title="Analytics" eyebrow="Learning evidence" description="Choose what to study next."
      actions={<Button>Refresh</Button>} />);
    const header = screen.getByRole("banner");
    const title = within(header).getByRole("heading", { level: 1, name: "Analytics" });
    expect(following(within(header).getByText("Learning evidence"), title)).toBe(true);
    expect(following(title, within(header).getByText("Choose what to study next."))).toBe(true);
    expect(following(title, within(header).getByRole("button", { name: "Refresh" }))).toBe(true);
  });

  it("heads a section at the level that fits the outline, and names it by id", () => {
    render(
      <section aria-labelledby="types-heading">
        <SectionHeading id="types-heading" level={3} eyebrow="Session analytics" title="Question types"
          description="Each board counts as one review." actions={<Button>Show as table</Button>} />
      </section>,
    );
    const region = screen.getByRole("region", { name: "Question types" });
    const title = within(region).getByRole("heading", { level: 3, name: "Question types" });
    expect(following(within(region).getByText("Session analytics"), title)).toBe(true);
    expect(following(title, within(region).getByText("Each board counts as one review."))).toBe(true);
    expect(following(title, within(region).getByRole("button", { name: "Show as table" }))).toBe(true);
  });

  it("titles a panel with the shared section heading, at level 3 unless the page needs 2", () => {
    render(
      <>
        <Panel title="Selected decks" description="2 decks">Cardiac physiology</Panel>
        <Panel headingLevel={2} headingId="theme-heading" title="Theme" />
      </>,
    );
    const nested = screen.getByRole("heading", { level: 3, name: "Selected decks" });
    expect(nested.closest("[data-slot='section-heading']")).not.toBeNull();
    const top = screen.getByRole("heading", { level: 2, name: "Theme" });
    expect(top.id).toBe("theme-heading");
    expect(top.closest("[data-slot='card']")?.querySelector("[data-slot='card-content']")).toBeNull();
  });
});

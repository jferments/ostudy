import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { StatDelta, StatTile } from "./StatTile";

describe("StatTile", () => {
  it("names the tile by its label and keeps the value and hint in reading order", () => {
    render(<StatTile label="Remembered" value="89%" hint="1,277 of 1,442 reviews" />);
    const tile = screen.getByRole("article", { name: "Remembered" });
    expect(tile.getAttribute("data-slot")).toBe("card");
    expect(tile.textContent).toBe("Remembered89%1,277 of 1,442 reviews");
    expect(tile.querySelector("[data-slot='stat-delta']")).toBeNull();
  });

  it("says which way a change went without relying on color", () => {
    render(<StatTile label="Remembered" value="89%" delta={{ label: "3 pts", direction: "up", tone: "positive" }} />);
    const delta = within(screen.getByRole("article", { name: "Remembered" })).getByText("3 pts");
    expect(delta.textContent).toBe("3 pts up");
    expect(delta.getAttribute("data-tone")).toBe("positive");
    expect(delta.querySelector("svg[data-icon='arrow-up']")?.getAttribute("aria-hidden")).toBe("true");
  });

  it("keeps direction and tone separate, so a fall can be good news", () => {
    render(<StatDelta label="4 cards" direction="down" tone="positive" />);
    const delta = screen.getByText("4 cards");
    expect(delta.textContent).toBe("4 cards down");
    expect(delta.querySelector("svg[data-icon='arrow-down']")).not.toBeNull();
    expect(delta.getAttribute("data-tone")).toBe("positive");
  });

  it("draws a plain hint pill when a change has no direction", () => {
    render(<StatDelta label="+64 this month" tone="positive" />);
    const delta = screen.getByText("+64 this month");
    expect(delta.textContent).toBe("+64 this month");
    expect(delta.querySelector("svg")).toBeNull();
  });

  it("places an optional visual after the hint", () => {
    render(
      <StatTile label="Study time" value="4.8 hr" hint="About 12 minutes on a study day">
        <svg role="img" aria-label="Minutes per day" />
      </StatTile>,
    );
    const tile = screen.getByRole("article", { name: "Study time" });
    const hint = within(tile).getByText("About 12 minutes on a study day");
    const visual = within(tile).getByRole("img", { name: "Minutes per day" });
    expect(hint.compareDocumentPosition(visual) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
  });
});

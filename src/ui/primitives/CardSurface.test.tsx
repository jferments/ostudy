import { readFileSync } from "node:fs";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Card } from "./Card";
import { Panel } from "./Panel";
import { StatTile } from "./StatTile";
import { Surface } from "./Surface";

const bridge = readFileSync("src/ui/tokens/generated/tailwind-theme.css", "utf8");
const themeCss = (theme: string) => readFileSync(`src/ui/tokens/generated/theme-${theme}.css`, "utf8");

/** Follow a rendered utility such as rounded-lg through the Tailwind bridge to the theme token and its value. */
function resolveUtility(element: HTMLElement, utility: "rounded" | "shadow", theme = "light") {
  const step = element.className.split(/\s+/u)
    .map((name) => new RegExp(`^tw:${utility}-([a-z]+)$`, "u").exec(name)?.[1])
    .find(Boolean);
  if (!step) throw new Error(`no ${utility} utility on ${element.className}`);
  const property = utility === "rounded" ? `--radius-${step}` : `--shadow-${step}`;
  const token = new RegExp(`${property}: var\\((--ostudy-[a-z0-9-]+)\\);`, "u").exec(bridge)?.[1];
  const value = token ? new RegExp(`${token}: ([^;]+);`, "u").exec(themeCss(theme))?.[1] : undefined;
  if (!token || !value) throw new Error(`${property} does not resolve in the ${theme} theme`);
  return { token, value };
}

describe("the shared card surface", () => {
  it("is the one surface under Panel, Surface and StatTile", () => {
    render(
      <>
        <Panel title="Daily workload">Limits</Panel>
        <Surface as="article" aria-label="Deck">Deck row</Surface>
        <StatTile label="Ready now" value={24} />
      </>,
    );
    const panel = screen.getByRole("heading", { name: "Daily workload" }).closest("section");
    for (const element of [panel, screen.getByRole("article", { name: "Deck" }), screen.getByRole("article", { name: "Ready now" })]) {
      expect(element?.getAttribute("data-slot")).toBe("card");
      expect(element?.querySelector("[data-slot='card']")).toBeNull();
    }
  });

  it("rounds at the mockups' 20 px card radius and casts their level-1 shadow", () => {
    render(<Card>Body</Card>);
    const card = screen.getByText("Body");
    for (const theme of ["light", "dark", "synthwave"]) {
      expect(resolveUtility(card, "rounded", theme).value).toBe("20px");
    }
    expect(resolveUtility(card, "rounded", "console").value).toBe("4px");
    expect(resolveUtility(card, "shadow").token).toBe("--ostudy-shadow-level-1");
  });

  it("gives small cards the medium radius", () => {
    render(<Card size="sm">Compact</Card>);
    expect(resolveUtility(screen.getByText("Compact"), "rounded").value).toBe("14px");
  });
});

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Progress } from "./Progress";

describe("Progress", () => {
  it("keeps standard text above the track without inline visual layers", () => {
    const { container } = render(
      <Progress value={2} max={5} label="Reading progress" text="2 of 5" />,
    );

    const progress = screen.getByRole("progressbar", { name: "Reading progress" });
    expect(screen.getByText("2 of 5").compareDocumentPosition(progress)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(container.querySelector("[data-progress-variant='standard']")).toBeTruthy();
    expect(container.querySelectorAll("[data-progress-content-layer]")).toHaveLength(0);
    expect(progress.hasAttribute("aria-valuetext")).toBe(false);
  });

  it("renders two aria-hidden inline content layers inside the track", () => {
    const { container } = render(
      <Progress
        value={3}
        max={8}
        label="Practice progress"
        variant="inline-content"
        inlineContent={{ start: "3 studied · 5 remaining", end: "38%" }}
        valueText="Card 4 of 8, 3 studied, 5 remaining, 38% complete"
      />,
    );

    const progress = screen.getByRole("progressbar", { name: "Practice progress" });
    expect(progress.getAttribute("data-progress-variant")).toBeNull();
    expect(container.querySelector("[data-progress-variant='inline-content']")).toBeTruthy();
    const layers = container.querySelectorAll("[data-progress-content-layer]");
    expect(layers).toHaveLength(2);
    expect(Array.from(layers).every((layer) => layer.getAttribute("aria-hidden") === "true"))
      .toBe(true);
    expect(screen.getAllByText("3 studied · 5 remaining")).toHaveLength(2);
    expect(screen.getAllByText("38%")).toHaveLength(2);
    expect(progress.getAttribute("aria-valuetext")).toBe(
      "Card 4 of 8, 3 studied, 5 remaining, 38% complete",
    );
  });

  it.each([
    { value: -4, max: 10, expectedMax: 10, expectedValue: 0, expectedFraction: 0 },
    { value: 14, max: 10, expectedMax: 10, expectedValue: 10, expectedFraction: 1 },
    { value: 4, max: 0, expectedMax: 0, expectedValue: 0, expectedFraction: 0 },
    { value: 0, max: 10, expectedMax: 10, expectedValue: 0, expectedFraction: 0 },
    { value: 4, max: 10, expectedMax: 10, expectedValue: 4, expectedFraction: 0.4 },
    { value: 10, max: 10, expectedMax: 10, expectedValue: 10, expectedFraction: 1 },
  ])(
    "keeps finite synchronized ARIA/CSS values for value $value and max $max",
    ({ value, max, expectedMax, expectedValue, expectedFraction }) => {
      render(<Progress value={value} max={max} label="Bounded progress" />);
      const progress = screen.getByRole("progressbar", { name: "Bounded progress" });
      expect(progress.getAttribute("aria-valuemin")).toBe("0");
      expect(progress.getAttribute("aria-valuemax")).toBe(String(expectedMax));
      expect(progress.getAttribute("aria-valuenow")).toBe(String(expectedValue));
      expect(progress.style.getPropertyValue("--progress-value")).toBe(
        String(expectedFraction),
      );
      expect(progress.style.getPropertyValue("--progress-percent")).toBe(
        `${expectedFraction * 100}%`,
      );
    },
  );

  it.each([
    { value: Number.NaN, max: 10 },
    { value: Number.POSITIVE_INFINITY, max: 10 },
    { value: Number.NEGATIVE_INFINITY, max: 10 },
    { value: 5, max: Number.NaN },
    { value: 5, max: Number.POSITIVE_INFINITY },
    { value: 5, max: Number.NEGATIVE_INFINITY },
  ])(
    "normalizes nonfinite value $value and max $max to coherent zero progress",
    ({ value, max }) => {
      render(<Progress value={value} max={max} label="Finite progress" />);
      const progress = screen.getByRole("progressbar", { name: "Finite progress" });
      expect(progress.getAttribute("aria-valuemax")).toBe(max === 10 ? "10" : "0");
      expect(progress.getAttribute("aria-valuenow")).toBe("0");
      expect(progress.style.getPropertyValue("--progress-value")).toBe("0");
      expect(progress.style.getPropertyValue("--progress-percent")).toBe("0%");
      expect(progress.outerHTML).not.toMatch(/NaN|Infinity/u);
    },
  );

  it.each(["accent", "success", "warning", "danger"] as const)(
    "applies the %s tone contract to inline content",
    (tone) => {
      render(
        <Progress
          value={1}
          max={2}
          label={`${tone} progress`}
          tone={tone}
          variant="inline-content"
          inlineContent={{ start: "1 studied", end: "50%" }}
        />,
      );
      expect(screen.getByRole("progressbar", { name: `${tone} progress` }).className)
        .toMatch(new RegExp(tone, "u"));
    },
  );
});

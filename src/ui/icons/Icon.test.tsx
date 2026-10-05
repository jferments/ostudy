import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ICON_NAMES, Icon } from "./Icon";

describe("Icon", () => {
  it("renders every registered semantic icon", () => {
    const { container } = render(
      <>
        {ICON_NAMES.map((name) => (
          <Icon key={name} name={name} />
        ))}
      </>,
    );

    expect(container.querySelectorAll("svg")).toHaveLength(ICON_NAMES.length);
    expect(ICON_NAMES).toEqual(
      expect.arrayContaining([
        "information",
        "calendar",
        "schedule",
        "book",
        "study-guide",
        "overflow",
        "maximize",
        "full-screen",
        "restore",
        "view-off",
        "minus",
        "zoom-in",
        "zoom-out",
      ]),
    );
    expect(new Set(ICON_NAMES).size).toBe(ICON_NAMES.length);
  });

  it("draws every icon on the Lucide grid with the shared stroke", () => {
    const { container } = render(
      <>
        {ICON_NAMES.map((name) => (
          <Icon key={name} name={name} />
        ))}
      </>,
    );

    for (const icon of container.querySelectorAll("svg")) {
      expect(icon.getAttribute("viewBox")).toBe("0 0 24 24");
      expect(icon.getAttribute("fill")).toBe("none");
      expect(icon.getAttribute("stroke-width")).toBe("1.9");
      expect(icon.children.length).toBeGreaterThan(0);
    }
  });

  it("is decorative by default and exposes an optional image label", () => {
    const { container, rerender } = render(<Icon name="information" />);
    expect(container.querySelector("svg")?.getAttribute("aria-hidden")).toBe("true");

    rerender(<Icon name="information" label="Information" />);
    const icon = container.querySelector("svg");
    expect(icon?.getAttribute("role")).toBe("img");
    expect(icon?.getAttribute("aria-label")).toBe("Information");
    expect(icon?.hasAttribute("aria-hidden")).toBe(false);
  });
});

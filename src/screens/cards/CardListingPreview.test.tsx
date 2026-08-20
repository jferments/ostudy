import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { CardListingPreview, formatCardListingText } from "./CardListingPreview";

describe("CardListingPreview", () => {
  it("renders question and answer as two ordered Q/A lines without an arrow", () => {
    const { container } = render(
      <CardListingPreview
        question="Why does ADH increase water reabsorption?"
        answer="It inserts aquaporin-2 channels in collecting-duct principal cells."
      />,
    );

    expect(screen.getByText("Q:")).toBeTruthy();
    expect(screen.getByText("A:")).toBeTruthy();
    expect(screen.getByText("Question:")).toBeTruthy();
    expect(screen.getByText("Answer:")).toBeTruthy();
    expect(container.textContent).not.toContain("→");

    const lines = container.querySelectorAll("strong");
    expect(lines).toHaveLength(2);
    expect(lines[0]?.textContent).toContain("ADH");
    expect(lines[1]?.textContent).toContain("aquaporin-2");
  });

  it("normalizes whitespace and preserves the requested empty fallback", () => {
    expect(formatCardListingText("  sodium\n  balance  ")).toBe("sodium balance");
    expect(formatCardListingText("   ", "Empty")).toBe("Empty");
  });
});

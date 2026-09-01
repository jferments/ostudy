import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { BrandLockup, BrandMark } from "./Brand";

afterEach(cleanup);

describe("Brand", () => {
  it("renders the supplied mascot image for the compact brand mark", () => {
    const { container } = render(<BrandMark />);
    const mark = container.querySelector('[data-artwork-slot="brand.logo.compact"]');
    const image = mark?.querySelector("img");

    expect(mark).not.toBeNull();
    expect(image).not.toBeNull();
    expect(image?.getAttribute("src")).toContain(encodeURI("racoon mascot (logo).png"));
    expect(image?.getAttribute("alt")).toBe("");
    expect(image?.getAttribute("draggable")).toBe("false");
  });

  it("preserves the ostudy lockup and removed motto", () => {
    const { container } = render(<BrandLockup compact />);

    expect(screen.getByLabelText("ostudy")).toBeTruthy();
    expect(container.querySelector('[data-artwork-slot="brand.logo.sidebar"]')).not.toBeNull();
    expect(container.textContent).toBe("ostudy");
    expect(container.textContent).not.toContain("Learn with intent");
  });
});

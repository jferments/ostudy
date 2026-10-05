import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { artworkManifest } from "../ui/artwork/artworkManifest";
import { BrandLockup, BrandMark } from "./Brand";

afterEach(cleanup);

describe("Brand", () => {
  it("shows the registered raccoon head as the brand mark", () => {
    const { container } = render(<BrandMark />);
    const mark = container.querySelector('[data-artwork-slot="brand.logo.compact"]');
    const image = mark?.querySelector("img");
    const registered = artworkManifest["brand.logo.compact"].image;

    expect(mark?.getAttribute("aria-hidden")).toBe("true");
    expect(registered).toBeDefined();
    expect(image?.getAttribute("src")).toBe(registered?.src);
    expect(image?.getAttribute("src")).toContain("raccoon-head-64.webp");
    expect(image?.getAttribute("srcset")).toBe(registered?.srcSet);
    expect(image?.getAttribute("srcset")).toContain("raccoon-head-128.webp 2x");
    expect(image?.getAttribute("alt")).toBe("");
    expect(image?.getAttribute("draggable")).toBe("false");
  });

  it("reads as the lowercase ostudy wordmark beside the mark, without the removed motto", () => {
    const { container } = render(<BrandLockup compact />);
    const lockup = screen.getByLabelText("ostudy");

    expect(lockup.getAttribute("data-artwork-slot")).toBe("brand.logo.sidebar");
    expect(lockup.querySelector('[data-artwork-slot="brand.logo.compact"] img')).not.toBeNull();
    expect(container.textContent).toBe("ostudy");
    expect(container.textContent).not.toContain("Learn with intent");
  });
});

import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { BrandLockup } from "./Brand";

function expectBrandIdentity(): void {
  const lockup = screen.getByLabelText("ostudy");
  expect(lockup.getAttribute("data-artwork-slot")).toBe("brand.logo.sidebar");
  expect(within(lockup).getByText("ostudy")).toBeTruthy();

  const mark = lockup.querySelector('[data-artwork-slot="brand.logo.compact"]');
  expect(mark).toBeTruthy();
  expect(mark?.getAttribute("aria-hidden")).toBe("true");
  expect(within(lockup).queryByText("Learn with intent")).toBeNull();
}

describe("BrandLockup", () => {
  it("preserves the ostudy identity without the tagline", () => {
    render(<BrandLockup />);

    expectBrandIdentity();
  });

  it("preserves the compact ostudy identity without the tagline", () => {
    render(<BrandLockup compact />);

    expectBrandIdentity();
  });
});

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Badge } from "./Badge";
import { Progress } from "./Progress";
import { Surface } from "./Surface";

describe("visual-system primitives", () => {
  it("exposes semantic surface state without leaking theme names or undefined classes", () => {
    render(<Surface level={2} material="translucent" selected>Content</Surface>);
    const surface = screen.getByText("Content");
    expect(surface.getAttribute("data-surface-level")).toBe("2");
    expect(surface.getAttribute("data-surface-material")).toBe("translucent");
    expect(surface.getAttribute("data-selected")).toBe("true");
    expect(surface.className.split(/\s+/u)).not.toContain("undefined");
  });

  it("bounds progress values while preserving the supplied maximum", () => {
    render(<Progress value={14} max={10} label="Study progress" text="10 of 10" />);
    const progress = screen.getByRole("progressbar", { name: "Study progress" });
    expect(progress.getAttribute("aria-valuenow")).toBe("10");
    expect(progress.getAttribute("aria-valuemax")).toBe("10");
  });

  it("renders semantic status text", () => {
    render(<Badge tone="success">Saved</Badge>);
    expect(screen.getByText("Saved")).toBeTruthy();
  });
});

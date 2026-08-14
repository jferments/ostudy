import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { CardStack } from "../materials/CardStack";
import { Badge } from "./Badge";
import { Progress } from "./Progress";
import { Surface } from "./Surface";

describe("visual-system primitives", () => {
  it("exposes semantic surface state without leaking theme names", () => {
    render(<Surface level={2} material="translucent" selected>Content</Surface>);
    const surface = screen.getByText("Content");
    expect(surface).toHaveAttribute("data-surface-level", "2");
    expect(surface).toHaveAttribute("data-surface-material", "translucent");
    expect(surface).toHaveAttribute("data-selected", "true");
  });

  it("bounds progress values while preserving the supplied maximum", () => {
    render(<Progress value={14} max={10} label="Study progress" text="10 of 10" />);
    const progress = screen.getByRole("progressbar", { name: "Study progress" });
    expect(progress).toHaveAttribute("aria-valuenow", "10");
    expect(progress).toHaveAttribute("aria-valuemax", "10");
  });

  it("keeps decorative stack layers outside the accessibility tree", () => {
    const { container } = render(<CardStack remaining={8}><article>Question</article></CardStack>);
    expect(screen.getByText("Question")).toBeInTheDocument();
    const layers = container.querySelectorAll("[aria-hidden='true']");
    expect(layers).toHaveLength(2);
  });

  it("renders semantic status text", () => {
    render(<Badge tone="success">Saved</Badge>);
    expect(screen.getByText("Saved")).toBeInTheDocument();
  });
});

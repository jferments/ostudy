import { describe, expect, it } from "vitest";
import { cn } from "./cn";

describe("namespaced Tailwind class composition", () => {
  it("resolves utility overrides without dropping a CSS Module class", () => {
    expect(cn("tw:px-4 module_action", false, "tw:px-6")).toBe("module_action tw:px-6");
  });
  it("keeps semantic font size separate from foreground color", () => {
    const result = cn("tw:text-label tw:text-primary-foreground", "tw:text-body");
    expect(result).toContain("tw:text-primary-foreground");
    expect(result).toContain("tw:text-body");
    expect(result).not.toContain("tw:text-label");
  });
  it("treats every type-scale size as a font size", () => {
    expect(cn("tw:text-small tw:text-muted-foreground", "tw:text-body-large")).toBe("tw:text-muted-foreground tw:text-body-large");
    expect(cn("tw:text-title-small", "tw:text-display")).toBe("tw:text-display");
  });
  it("lets a caller replace theme-owned radius and shadow utilities", () => {
    expect(cn("tw:rounded-button tw:shadow-glow", "tw:rounded-lg tw:shadow-none")).toBe("tw:rounded-lg tw:shadow-none");
    expect(cn("tw:shadow-panel tw:shadow-primary/20", "tw:shadow-overlay")).toBe("tw:shadow-primary/20 tw:shadow-overlay");
  });
});

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
});

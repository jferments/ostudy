import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

const merge = extendTailwindMerge({
  prefix: "tw",
  extend: {
    // Theme-owned radius and shadow names, so caller overrides replace them instead of stacking.
    theme: {
      radius: ["button"],
      shadow: ["panel", "overlay", "focus", "glow"],
    },
    classGroups: {
      "font-size": [{ text: ["label", "caption", "body", "section", "title"] }],
    },
  },
});

/** Resolve utility conflicts, while retaining caller-owned CSS Module names. */
export function cn(...values: ClassValue[]): string {
  return merge(clsx(values));
}

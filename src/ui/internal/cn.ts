import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

const merge = extendTailwindMerge({
  prefix: "tw",
  extend: {
    // Token-owned type sizes, radius and shadow names, so caller overrides replace them
    // instead of stacking (and text sizes are never mistaken for text colours).
    theme: {
      text: ["caption", "label", "small", "body", "body-large", "title-small", "section", "title", "display"],
      radius: ["button"],
      shadow: ["panel", "overlay", "focus", "glow"],
    },
  },
});

/** Resolve utility conflicts, while retaining caller-owned CSS Module names. */
export function cn(...values: ClassValue[]): string {
  return merge(clsx(values));
}

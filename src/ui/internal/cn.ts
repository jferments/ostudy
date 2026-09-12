import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

const merge = extendTailwindMerge({
  prefix: "tw",
  extend: {
    classGroups: {
      "font-size": [{ text: ["label", "caption", "body", "section", "title"] }],
    },
  },
});

/** Resolve utility conflicts, while retaining caller-owned CSS Module names. */
export function cn(...values: ClassValue[]): string {
  return merge(clsx(values));
}

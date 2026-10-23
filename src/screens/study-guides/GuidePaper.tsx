import type { CSSProperties, ReactNode } from "react";

import type { StudyGuidePdfTheme } from "../../ipc/studyGuides";
import { cn } from "../../ui/internal/cn";

// Guide paper keeps its theme's colors whatever the app's theme, as notebook paper does
// (notes/paper.tsx); they are the exported PDF's colors, from typst/themes.typ.
const paperThemes: Record<StudyGuidePdfTheme, Record<string, string>> = {
  navy: { primary: "#1d2b4f", accent: "#2f6690", "accent-soft": "#e8f0f6", band: "#8fb3d1", highlight: "#e8b04a",
    "highlight-soft": "#fbf3e1", "highlight-ink": "#8a5d0f", warn: "#b8513c", "warn-soft": "#f9ece8", good: "#2e6e52",
    "good-soft": "#eaf3ee", ink: "#1c2130", muted: "#646b7a", rule: "#d5dae2", paper: "#ffffff" },
  sage: { primary: "#4a6b57", accent: "#4a6b57", "accent-soft": "#f1eadb", band: "#d6e2d6", highlight: "#b0614a",
    "highlight-soft": "#f5e6dd", "highlight-ink": "#8e4633", warn: "#a8432f", "warn-soft": "#f6e1d8", good: "#3f6a52",
    "good-soft": "#e5ece3", ink: "#2d2a26", muted: "#716b60", rule: "#d9d0bd", paper: "#fbf7ef" },
  slate: { primary: "#263241", accent: "#1f7a7a", "accent-soft": "#e3f1f0", band: "#9fd3cf", highlight: "#e07a5f",
    "highlight-soft": "#fcece6", "highlight-ink": "#a4472f", warn: "#b23a48", "warn-soft": "#f8e5e8", good: "#2f7d4f",
    "good-soft": "#e6f2ea", ink: "#1d232c", muted: "#5e6875", rule: "#d3d9df", paper: "#ffffff" },
  plum: { primary: "#3f2a4a", accent: "#7b4b8a", "accent-soft": "#f2ebf5", band: "#cdb6d6", highlight: "#d4a03c",
    "highlight-soft": "#fbf3e2", "highlight-ink": "#87600f", warn: "#b6455a", "warn-soft": "#f9e6ea", good: "#3d7a6a",
    "good-soft": "#e5f1ee", ink: "#241c29", muted: "#6b6372", rule: "#ddd4e2", paper: "#fffdfb" },
};

/**
 * A page of guide paper: the theme's paper and ink whatever the app's theme, as notebook pages
 * are, under the theme's colored band.
 */
export function GuidePaper({ theme, className, children }: { theme: StudyGuidePdfTheme; className?: string; children: ReactNode }) {
  const style = Object.fromEntries(Object.entries(paperThemes[theme]).map(([name, value]) => [`--guide-${name}`, value])) as CSSProperties;
  return (
    <div data-guide-paper={theme} style={style}
      className={cn("tw:relative tw:min-w-0 tw:overflow-hidden tw:rounded-lg tw:bg-(--guide-paper) tw:text-(--guide-ink) tw:shadow-md tw:ring-1 tw:ring-(--guide-rule)",
        "tw:px-10 tw:pt-11 tw:pb-10 tw:compact:px-5 tw:compact:pt-8 tw:compact:pb-6", className)}>
      <div aria-hidden="true" className="tw:absolute tw:inset-x-0 tw:top-0 tw:h-2 tw:bg-(--guide-primary)" />
      <div aria-hidden="true" className="tw:absolute tw:top-2 tw:left-0 tw:h-1 tw:w-1/4 tw:bg-(--guide-highlight)" />
      {children}
    </div>
  );
}


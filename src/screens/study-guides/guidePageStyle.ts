import type { StudyGuidePdfTheme } from "../../ipc/studyGuides";

/** How a guide reads: as its PDF pages in a theme, on the canvas, or as plain text. */
export type GuidePageStyle = "plain" | StudyGuidePdfTheme;

const STYLE_STORAGE_KEY = "ostudy.guidePageStyle";
export const GUIDE_PAGE_STYLES: ReadonlyArray<{ value: GuidePageStyle; name: string }> = [
  { value: "plain", name: "Plain" },
  { value: "navy", name: "Navy" },
  { value: "sage", name: "Sage" },
  { value: "slate", name: "Slate" },
  { value: "plum", name: "Plum" },
];

/** The page style guides were last read in on this device; Navy until one is chosen. */
export function readGuidePageStyle(): GuidePageStyle {
  try {
    const stored = window.localStorage.getItem(STYLE_STORAGE_KEY);
    return GUIDE_PAGE_STYLES.find((style) => style.value === stored)?.value ?? "navy";
  } catch {
    return "navy";
  }
}

export function saveGuidePageStyle(style: GuidePageStyle): void {
  try {
    window.localStorage.setItem(STYLE_STORAGE_KEY, style);
  } catch {
    // The style is a convenience; Navy works without storage.
  }
}

import type { StudyGuidePdfTheme } from "../../ipc/studyGuides";

/** How a guide reads: as its pages in the study guide theme, on the canvas, or as plain text. */
export type GuidePageStyle = "plain" | StudyGuidePdfTheme;

const STYLE_STORAGE_KEY = "ostudy.guidePageStyle";
export const GUIDE_PAGE_STYLES: ReadonlyArray<{ value: GuidePageStyle; name: string }> = [
  { value: "plain", name: "Plain" },
  { value: "navy", name: "Navy" },
  { value: "sage", name: "Sage" },
  { value: "slate", name: "Slate" },
  { value: "plum", name: "Plum" },
];

/**
 * Whether guides are read as plain text on this device. The theme of their pages is the study
 * guide theme in Settings; a theme stored here by an older build only means pages.
 */
export function readsGuidesAsPlainText(): boolean {
  try {
    return window.localStorage.getItem(STYLE_STORAGE_KEY) === "plain";
  } catch {
    return false;
  }
}

export function saveReadsGuidesAsPlainText(plain: boolean): void {
  try {
    window.localStorage.setItem(STYLE_STORAGE_KEY, plain ? "plain" : "pages");
  } catch {
    // The choice is a convenience; pages work without storage.
  }
}

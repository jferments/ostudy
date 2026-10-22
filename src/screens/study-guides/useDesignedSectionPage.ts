import { useEffect, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getDocumentStudyGuideSectionPage, type StudyGuidePdfTheme } from "../../ipc/studyGuides";

/** How guide sections read: as a designed page in a PDF theme, or as plain text. */
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

/** A section laid out as a designed page: its picture and its size in points. */
export interface SectionPagePicture {
  url: string;
  width: number;
  height: number;
}

/** The designed page, `"loading"` while it is laid out, or null for the plain page. */
export type DesignedSectionPage = SectionPagePicture | "loading" | null;

/** The page size an SVG's view box gives, in points. */
function svgSize(svg: string): { width: number; height: number } | null {
  const match = /viewBox="0 0 ([\d.]+) ([\d.]+)"/.exec(svg);
  const width = Number(match?.[1]);
  const height = Number(match?.[2]);
  return width > 0 && height > 0 ? { width, height } : null;
}

/**
 * One section laid out natively as a designed page in `theme`, as an image URL. A newer
 * section, theme or guide text replaces an older read, the image is released when it is
 * replaced or the page closes, and a layout failure gives the plain page.
 */
export function useDesignedSectionPage(
  documentId: string,
  topicId: string,
  theme: StudyGuidePdfTheme | null,
  /** Changes whenever the section's generated text does. */
  contentKey: string,
): DesignedSectionPage {
  const lane = useRequestLane();
  const [page, setPage] = useState<{ request: string; picture: SectionPagePicture | null } | null>(null);
  const request = theme === null ? null : `${documentId}:${topicId}:${theme}:${contentKey}`;

  useEffect(() => {
    if (theme === null || request === null) return;
    let live = true;
    let made: string | null = null;
    void latestResult(lane, () => getDocumentStudyGuideSectionPage(documentId, topicId, theme)).then(
      (value) => {
        if (value === undefined || !live) return;
        const size = svgSize(value.svg);
        if (size === null) {
          setPage({ request, picture: null });
          return;
        }
        made = URL.createObjectURL(new Blob([value.svg], { type: "image/svg+xml" }));
        setPage({ request, picture: { url: made, ...size } });
      },
      () => {
        if (live) setPage({ request, picture: null });
      },
    );
    return () => {
      live = false;
      if (made !== null) URL.revokeObjectURL(made);
    };
  }, [lane, documentId, topicId, theme, request]);

  if (request === null) return null;
  if (page?.request !== request) return "loading";
  return page.picture;
}

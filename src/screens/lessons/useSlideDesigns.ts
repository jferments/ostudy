import { useEffect, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import { getLessonSlideDesigns } from "../../ipc/lessons";
import type { StudyGuidePdfTheme } from "../../ipc/studyGuides";

const THEME_STORAGE_KEY = "ostudy.lectureSlideTheme";
const THEMES: readonly StudyGuidePdfTheme[] = ["navy", "sage", "slate", "plum"];

/** The theme slides were last shown in on this device; Navy until one is chosen. */
export function readSlideTheme(): StudyGuidePdfTheme {
  try {
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
    return THEMES.find((theme) => theme === stored) ?? "navy";
  } catch {
    return "navy";
  }
}

export function saveSlideTheme(theme: StudyGuidePdfTheme): void {
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // The theme is a convenience; Navy works without storage.
  }
}

export interface SlideDesignImages {
  /** Each laid-out slide's image URL by slide key. */
  images: ReadonlyMap<string, string>;
  /** Why the designed slides could not be laid out; the plain slides show instead. */
  error: string | null;
}

const NONE: SlideDesignImages = { images: new Map(), error: null };

/**
 * The lecture's slides laid out natively in `theme`, as image URLs the slide
 * stage and strip show. A newer theme or revision replaces an older read, and
 * the images are released when they are replaced or the view closes.
 */
export function useSlideDesigns(lessonId: string, revisionNumber: number, theme: StudyGuidePdfTheme): SlideDesignImages {
  const lane = useRequestLane();
  const [designs, setDesigns] = useState<SlideDesignImages>(NONE);

  useEffect(() => {
    let live = true;
    let made: string[] = [];
    void latestResult(lane, () => getLessonSlideDesigns(lessonId, revisionNumber, theme)).then(
      (value) => {
        if (value === undefined || !live) return;
        const images = new Map(
          value.slides.map((slide) => [slide.key, URL.createObjectURL(new Blob([slide.svg], { type: "image/svg+xml" }))]),
        );
        made = [...images.values()];
        setDesigns({ images, error: null });
      },
      (error: unknown) => {
        if (live) setDesigns({ images: new Map(), error: getErrorMessage(error) });
      },
    );
    return () => {
      live = false;
      made.forEach((url) => URL.revokeObjectURL(url));
    };
  }, [lane, lessonId, revisionNumber, theme]);

  return designs;
}

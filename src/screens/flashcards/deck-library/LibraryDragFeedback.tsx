import type { RefCallback } from "react";

import { VisuallyHidden } from "../../../ui/primitives/VisuallyHidden";
import styles from "./LibraryDragFeedback.module.css";
import type { PointerDragFeedbackState } from "./useDeckLibraryPointerSession";

export function LibraryDragFeedback({
  feedback,
  bindPreview,
}: {
  feedback: PointerDragFeedbackState | null;
  bindPreview: RefCallback<HTMLDivElement>;
}) {
  const announcement = feedback?.candidate?.announcement ?? "";
  const initialX = typeof window === "undefined" ? 16 : Math.max(16, window.innerWidth - 336);
  const previewRef: RefCallback<HTMLDivElement> = (element) => {
    if (element && !element.style.transform) {
      // The pointer session normally repositions this node on its next animation
      // frame. Give the newly mounted preview an immediate visible location so
      // activation feedback cannot disappear at the viewport origin when no
      // further pointer event arrives.
      element.style.transform = `translate3d(${initialX}px, 16px, 0)`;
    }
    bindPreview(element);
  };

  return (
    <>
      {feedback?.active && feedback.candidate ? (
        <div
          ref={previewRef}
          className={styles.preview}
          data-drag-preview
          data-status={feedback.candidate.status}
          aria-hidden="true"
        >
          <strong>{feedback.sourceName}</strong>
          <span>{feedback.candidate.destinationName}</span>
          <small>{feedback.candidate.insertionLabel}</small>
        </div>
      ) : null}
      <VisuallyHidden as="div" aria-live="polite" aria-atomic="true">
        {announcement}
      </VisuallyHidden>
    </>
  );
}

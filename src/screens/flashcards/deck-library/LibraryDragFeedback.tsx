import type { RefCallback } from "react";

import { Icon } from "../../../ui/icons/Icon";
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
  const candidate = feedback?.candidate ?? null;
  const announcement = candidate?.announcement ?? "";
  const initialX = typeof window === "undefined" ? 16 : Math.max(16, window.innerWidth - 436);
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
      {feedback?.active ? (
        <div
          ref={previewRef}
          className={styles.preview}
          data-drag-preview
          data-drag-panel-preview="true"
          data-status={candidate?.status ?? "pending"}
          aria-hidden="true"
        >
          <span className={styles.handle}>
            <Icon name="move" size={22} />
          </span>
          <span className={styles.copy}>
            <small className={styles.eyebrow}>Moving</small>
            <strong>{feedback.sourceName}</strong>
            <span className={styles.destination}>
              {candidate
                ? `${candidate.destinationName} · ${candidate.insertionLabel}`
                : "Choose a destination"}
            </span>
          </span>
        </div>
      ) : null}
      <VisuallyHidden as="div" aria-live="polite" aria-atomic="true">
        {announcement}
      </VisuallyHidden>
    </>
  );
}

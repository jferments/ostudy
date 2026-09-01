import { memo, type RefCallback } from "react";

import { VisuallyHidden } from "../../../ui/primitives/VisuallyHidden";
import type { PointerDragFeedbackState } from "./useDeckLibraryPointerSession";
import styles from "./LibraryDragFeedback.module.css";

interface LibraryDragFeedbackProps {
  feedback: PointerDragFeedbackState;
  bindPreview: RefCallback<HTMLDivElement>;
}

export const LibraryDragFeedback = memo(function LibraryDragFeedback({
  feedback,
  bindPreview,
}: LibraryDragFeedbackProps) {
  const candidate = feedback.candidate;
  return (
    <>
      {feedback.active ? (
        <div
          ref={bindPreview}
          className={styles.preview}
          data-drag-preview="true"
          data-status={candidate?.status ?? "pending"}
          aria-hidden="true"
        >
          <strong>{feedback.sourceName}</strong>
          <span>{candidate?.destinationName ?? "Choose a destination"}</span>
          <small>{candidate?.insertionLabel ?? "Move to activate"}</small>
        </div>
      ) : null}
      <VisuallyHidden as="div" aria-live="polite" aria-atomic="true">
        {candidate?.announcement ?? ""}
      </VisuallyHidden>
    </>
  );
});

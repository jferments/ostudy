import type { RefCallback } from "react";

import { VisuallyHidden } from "../../../ui/primitives/VisuallyHidden";
import styles from "./LibraryDragFeedback.module.css";
import type { PointerDragFeedbackState } from "./useDeckLibraryPointerSession";

/**
 * Accessible status for the pointer session.
 *
 * The visible drag preview is an exact DOM clone owned by the pointer-session
 * controller, not a second React approximation of the row. `bindPreview`
 * remains as an optional compatibility prop while older render-isolation
 * fixtures migrate away from the detached-preview contract.
 */
export function LibraryDragFeedback({
  feedback,
}: {
  feedback: PointerDragFeedbackState | null;
  bindPreview?: RefCallback<HTMLDivElement>;
}) {
  const candidate = feedback?.candidate ?? null;
  const announcement = candidate?.announcement ?? "";
  return (
    <>
      {feedback?.active && candidate ? (
        <div
          className={styles.destination}
          data-drag-destination-feedback
          data-status={candidate.status}
          aria-hidden="true"
        >
          {candidate.destinationName} · {candidate.insertionLabel}
        </div>
      ) : null}
      <VisuallyHidden as="div" aria-live="polite" aria-atomic="true">
        {announcement}
      </VisuallyHidden>
    </>
  );
}

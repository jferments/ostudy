import { VisuallyHidden } from "../../../ui/primitives/VisuallyHidden";
import type { PointerDragFeedbackState } from "./useDeckLibraryPointerSession";

export function LibraryDragFeedback({
  feedback,
}: {
  feedback: PointerDragFeedbackState | null;
}) {
  return (
    <VisuallyHidden as="div" aria-live="polite" aria-atomic="true">
      {feedback?.candidate?.announcement ?? ""}
    </VisuallyHidden>
  );
}

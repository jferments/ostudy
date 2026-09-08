import type { TutorTarget } from "../../ipc/tutor";
import { Button } from "../../ui/primitives/Button";
import { useOptionalTutor } from "./TutorContext";

/** The same contextual entry works in Practice and the study-guide reader. */
export function TutorEntryButton({ target, disabled = false, className }: {
  target: TutorTarget;
  disabled?: boolean;
  className?: string;
}) {
  const tutor = useOptionalTutor();
  if (!tutor) return null;
  const label = target.mode === "hint" ? "Get a flashcard hint" : "Ask tutor about this material";
  return (
    <Button size="icon" className={className} aria-label={label} title={label}
      disabled={disabled || tutor.busy || tutor.pending !== null}
      onClick={() => tutor.openContext(target)}>
      <svg width="21" height="21" viewBox="0 0 24 24" fill="none" stroke="currentColor"
        strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">
        <path d="M21 11.5a8.5 8.5 0 0 1-8.5 8.5H8l-5 3v-6a8.5 8.5 0 1 1 18-5.5Z" />
        <path d="M8 10h8M8 14h5" />
      </svg>
    </Button>
  );
}

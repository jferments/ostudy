import type { TutorTarget } from "../../ipc/tutor";
import { Icon } from "../../ui/icons/Icon";
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
      <Icon name="chat" size={21} />
    </Button>
  );
}

import { useAiFeatures } from "../../controllers/AiFeaturesProvider";
import type { TutorTarget } from "../../ipc/tutor";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { useOptionalTutor } from "./TutorContext";

/** The same contextual entry works in Practice and the study-guide reader; it hides while AI is off. */
export function TutorEntryButton({ target, disabled = false, className, showLabel = false, variant = "secondary" }: {
  target: TutorTarget;
  disabled?: boolean;
  className?: string;
  /** Show "Ask Tutor" beside the icon, as a toolbar with room for words does. */
  showLabel?: boolean;
  /** A ghost button sits quietly among other icon actions. */
  variant?: "secondary" | "ghost";
}) {
  const tutor = useOptionalTutor();
  const ai = useAiFeatures().enabled;
  if (!tutor || !ai) return null;
  const label = target.mode === "hint" ? "Get a flashcard hint" : "Ask tutor about this material";
  // The visible words stay inside the accessible name, so speech input finds the button by them.
  const shortLabel = target.mode === "hint" ? "Flashcard hint" : "Ask Tutor";
  return (
    <Button size={showLabel ? "medium" : "icon"} variant={variant} className={className} aria-label={label}
      title={showLabel ? undefined : label}
      disabled={disabled || tutor.busy || tutor.pending !== null}
      onClick={() => tutor.openContext(target)}>
      <Icon name="chat" size={showLabel ? 18 : 21} />
      {showLabel ? shortLabel : null}
    </Button>
  );
}

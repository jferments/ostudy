import { generationInstructionsError } from "../../ipc/generationInstructions";
import { Disclosure, Textarea } from "../../ui/primitives/FormControls";
import { FormField } from "../../ui/primitives/FormField";

interface GenerationInstructionsFieldProps {
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  compact?: boolean;
  label?: string;
}

/** Editing a draft never changes source evidence or a running native request. */
export function GenerationInstructionsField({
  value, onChange, disabled = false, compact = false,
  label = "Flashcard instructions (optional)",
}: GenerationInstructionsFieldProps) {
  const field = <FormField label={label} error={generationInstructionsError(value)}
    description={disabled
      ? "The current job keeps its captured instructions. Change them for the next generation after it finishes."
      : "Describe the flashcards' focus, audience, or style. The source remains the evidence; these instructions do not control study-guide generation."}>
    <Textarea value={value} disabled={disabled} rows={3}
      placeholder="For each cranial nerve in these materials, create concise recall cards about tests nurses use to assess its function."
      onChange={(event) => onChange(event.currentTarget.value)} />
  </FormField>;
  return compact ? <Disclosure
    title={`${label}${value.trim() ? " · customized" : ""}`}
    className="tw:border-0 tw:py-2">
    <div className="tw:pt-3">{field}</div>
  </Disclosure> : field;
}

import { useId, useState, type KeyboardEvent } from "react";
import type { TagSummary } from "../ipc/cards";
import { TextField } from "../ui/primitives/TextField";
import { ChipButton } from "../ui/primitives/Chip";

interface TagInputProps {
  label?: string;
  tags: string[];
  suggestions: TagSummary[];
  disabled?: boolean;
  onChange: (tags: string[]) => void;
}

/** Tags are domain values; input, focus and removable-chip presentation stay shared. */
export function TagInput({ label = "Tags", tags, suggestions, disabled = false, onChange }: TagInputProps) {
  const [draft, setDraft] = useState("");
  const id = useId();
  const listId = `${id}-suggestions`;
  const helpId = `${id}-help`;

  function addDraft(): void {
    if (disabled) return;
    const next = draft.trim().replace(/\s+/gu, " ");
    setDraft("");
    if (!next || tags.some((tag) => tag.toLocaleLowerCase() === next.toLocaleLowerCase())) return;
    onChange([...tags, next]);
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
    if (disabled || event.nativeEvent.isComposing) return;
    if (event.key === "Enter" || event.key === ",") {
      event.preventDefault();
      addDraft();
    } else if (event.key === "Backspace" && draft === "" && tags.length > 0) {
      onChange(tags.slice(0, -1));
    }
  }

  return <div className="tw:grid tw:min-w-0 tw:gap-2">
    <label htmlFor={id} className="tw:text-sm tw:font-medium">{label}</label>
    {tags.length > 0 ? <div className="tw:flex tw:flex-wrap tw:gap-2" aria-label="Selected tags">
      {tags.map((tag) => <ChipButton key={tag.toLocaleLowerCase()} tone="accent" disabled={disabled}
        aria-label={`Remove ${tag} tag`} onClick={() => onChange(tags.filter((value) => value !== tag))}>
        {tag}<span aria-hidden="true">×</span>
      </ChipButton>)}
    </div> : null}
    <TextField id={id} aria-label={label} value={draft} disabled={disabled} list={listId} maxLength={64}
      placeholder={tags.length === 0 ? "Add tags" : "Add another"} aria-describedby={helpId}
      onChange={(event) => setDraft(event.currentTarget.value)} onKeyDown={onKeyDown} onBlur={addDraft} />
    <datalist id={listId}>
      {suggestions.filter((suggestion) => !tags.some((tag) => tag.toLocaleLowerCase() === suggestion.name.toLocaleLowerCase()))
        .map((suggestion) => <option key={suggestion.id} value={suggestion.name}>{suggestion.generatedCardCount} generated cards</option>)}
    </datalist>
    <small id={helpId} className="tw:text-xs tw:text-muted-foreground">Press Enter or comma after each tag.</small>
  </div>;
}

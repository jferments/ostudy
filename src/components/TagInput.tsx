import { useId, useState } from "react";
import type { ChangeEvent, KeyboardEvent } from "react";

import {
  type TagSummary,
} from "../ipc/cards";

interface TagInputProps {
  label?: string;
  tags: string[];
  suggestions: TagSummary[];
  disabled?: boolean;
  onChange: (tags: string[]) => void;
}

export function TagInput({
  label = "Tags",
  tags,
  suggestions,
  disabled = false,
  onChange,
}: TagInputProps) {
  const [draft, setDraft] = useState("");
  const listId = useId();

  function addDraft(): void {
    const next = normalizeTag(draft);
    setDraft("");
    if (next === "") return;
    if (tags.some((tag) => tag.toLocaleLowerCase() === next.toLocaleLowerCase())) return;
    onChange([...tags, next]);
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
    if (event.key === "Enter" || event.key === ",") {
      event.preventDefault();
      addDraft();
      return;
    }
    if (event.key === "Backspace" && draft === "" && tags.length > 0) {
      onChange(tags.slice(0, -1));
    }
  }

  return (
    <label className="tag-input-control">
      <span>{label}</span>
      <div className="tag-input-surface">
        {tags.map((tag) => (
          <span key={tag.toLocaleLowerCase()} className="tag-chip">
            {tag}
            <button
              type="button"
              aria-label={`Remove ${tag} tag`}
              disabled={disabled}
              onClick={() => onChange(tags.filter((value) => value !== tag))}
            >
              ×
            </button>
          </span>
        ))}
        <input
          value={draft}
          disabled={disabled}
          list={listId}
          maxLength={64}
          placeholder={tags.length === 0 ? "Add tags" : "Add another"}
          aria-label={label}
          onChange={(event: ChangeEvent<HTMLInputElement>) => setDraft(event.currentTarget.value)}
          onKeyDown={onKeyDown}
          onBlur={addDraft}
        />
        <datalist id={listId}>
          {suggestions
            .filter(
              (suggestion) =>
                !tags.some(
                  (tag) => tag.toLocaleLowerCase() === suggestion.name.toLocaleLowerCase(),
                ),
            )
            .map((suggestion) => (
              <option key={suggestion.id} value={suggestion.name}>
                {suggestion.generatedCardCount} generated cards
              </option>
            ))}
        </datalist>
      </div>
      <small>Press Enter or comma after each tag.</small>
    </label>
  );
}

function normalizeTag(value: string): string {
  return value.trim().replace(/\s+/gu, " ");
}

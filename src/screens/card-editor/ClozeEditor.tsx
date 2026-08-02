import { useMemo, useRef, useState } from "react";
import type { ChangeEvent } from "react";

import type { ClozeMode } from "../../ipc";
import {
  clearClozeMarkers,
  nextClozeGroup,
  normalizeClozeMode,
  parseClozeMarkup,
  renderClozeQuestion,
} from "./cloze";

interface ClozeEditorProps {
  value: string;
  mode: ClozeMode;
  disabled: boolean;
  onChange: (value: string) => void;
  onModeChange: (mode: ClozeMode, value: string) => void;
}

export function ClozeEditor({
  value,
  mode,
  disabled,
  onChange,
  onModeChange,
}: ClozeEditorProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [selectionError, setSelectionError] = useState<string | null>(null);
  const parsed = useMemo(() => parseClozeMarkup(value), [value]);
  const previewGroup = parsed.groups[0] ?? 1;

  function markSelection(): void {
    const textarea = textareaRef.current;
    if (!textarea || disabled) return;
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    const selected = value.slice(start, end);
    if (start === end || selected.trim() === "") {
      setSelectionError("Select the word or phrase that should become a blank.");
      textarea.focus();
      return;
    }
    if (selected.includes("{{") || selected.includes("}}")) {
      setSelectionError("Choose text that is not already inside a blank marker.");
      textarea.focus();
      return;
    }
    const group = nextClozeGroup(value, mode);
    const marker = `{{c${group}::${selected}}}`;
    const next = `${value.slice(0, start)}${marker}${value.slice(end)}`;
    onChange(next);
    setSelectionError(null);
    window.requestAnimationFrame(() => {
      textarea.focus();
      textarea.setSelectionRange(start, start + marker.length);
    });
  }

  function changeMode(event: ChangeEvent<HTMLInputElement>): void {
    const nextMode = event.currentTarget.value as ClozeMode;
    onModeChange(nextMode, normalizeClozeMode(value, nextMode));
    setSelectionError(null);
  }

  return (
    <section className="cloze-editor" aria-labelledby="cloze-editor-heading">
      <div className="editor-field-heading">
        <div>
          <h3 id="cloze-editor-heading">Sentence or passage</h3>
          <p>
            Select text in the editor, then mark it as a blank. The visible markers are
            stored as data and are never evaluated as code.
          </p>
        </div>
      </div>

      <textarea
        ref={textareaRef}
        value={value}
        disabled={disabled}
        rows={9}
        aria-label="Fill in the blank text"
        placeholder="ACE inhibitors block the enzyme that creates angiotensin II."
        onChange={(event) => {
          onChange(event.currentTarget.value);
          setSelectionError(null);
        }}
      />
      <div className="cloze-toolbar">
        <button type="button" className="secondary-button" disabled={disabled} onClick={markSelection}>
          Mark selection as blank
        </button>
        <button
          type="button"
          className="text-button"
          disabled={disabled || parsed.groups.length === 0}
          onClick={() => onChange(clearClozeMarkers(value))}
        >
          Clear blank markers
        </button>
      </div>
      {selectionError ? <p className="field-error">{selectionError}</p> : null}
      {!parsed.valid && value.includes("{{") ? (
        <p className="field-error">
          A blank marker is incomplete. Use the Mark selection button rather than typing
          marker braces manually.
        </p>
      ) : null}

      <fieldset className="cloze-mode-fieldset" disabled={disabled}>
        <legend>How should the blanks become cards?</legend>
        <label>
          <input
            type="radio"
            name="cloze-mode"
            value="one_at_a_time"
            checked={mode === "one_at_a_time"}
            onChange={changeMode}
          />
          <span>
            <strong>One blank per card</strong>
            <small>Each marked region gets its own card; other marked text remains visible.</small>
          </span>
        </label>
        <label>
          <input
            type="radio"
            name="cloze-mode"
            value="all_together"
            checked={mode === "all_together"}
            onChange={changeMode}
          />
          <span>
            <strong>Hide all blanks together</strong>
            <small>Every marked region is hidden on one card.</small>
          </span>
        </label>
      </fieldset>

      {parsed.groups.length > 0 && parsed.valid ? (
        <div className="cloze-preview" aria-live="polite">
          <div>
            <span>Preview</span>
            <strong>
              {mode === "all_together" ? 1 : parsed.groups.length}{" "}
              {(mode === "all_together" ? 1 : parsed.groups.length) === 1 ? "card" : "cards"} generated
            </strong>
          </div>
          <p>{renderClozeQuestion(parsed, mode === "all_together" ? null : previewGroup, mode)}</p>
          {mode === "one_at_a_time" && parsed.groups.length > 1 ? (
            <small>Previewing the first blank; each additional blank generates another card.</small>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

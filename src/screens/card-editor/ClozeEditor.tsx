import { useMemo, useRef, useState } from "react";

import type { ClozeMode } from "../../ipc";
import {
  distinctClozeGroups,
  markSelectionAsBlank,
  removeAllClozeMarkers,
  renderClozePreview,
} from "./cloze";

interface ClozeEditorProps {
  source: string;
  mode: ClozeMode;
  disabled: boolean;
  onSourceChange: (source: string) => void;
  onModeChange: (mode: ClozeMode) => void;
}

export function ClozeEditor({
  source,
  mode,
  disabled,
  onSourceChange,
  onModeChange,
}: ClozeEditorProps) {
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const [selectionError, setSelectionError] = useState<string | null>(null);
  const groups = useMemo(() => distinctClozeGroups(source), [source]);
  const [previewGroup, setPreviewGroup] = useState<number | null>(null);
  const effectivePreviewGroup =
    mode === "all_together"
      ? null
      : groups.includes(previewGroup ?? -1)
        ? previewGroup
        : groups[0] ?? null;
  const preview = useMemo(
    () => renderClozePreview(source, effectivePreviewGroup),
    [effectivePreviewGroup, source],
  );

  function markSelection(): void {
    const textarea = textareaRef.current;
    if (!textarea) return;
    try {
      const result = markSelectionAsBlank(
        source,
        textarea.selectionStart,
        textarea.selectionEnd,
      );
      setSelectionError(null);
      onSourceChange(result.source);
      window.requestAnimationFrame(() => {
        textarea.focus();
        textarea.setSelectionRange(result.selectionStart, result.selectionEnd);
      });
    } catch (error: unknown) {
      setSelectionError(
        error instanceof Error ? error.message : "The selected text could not be marked.",
      );
    }
  }

  return (
    <section className="cloze-editor" aria-labelledby="cloze-editor-heading">
      <div className="section-heading compact-section-heading">
        <div>
          <h2 id="cloze-editor-heading">Fill in the blank</h2>
          <p>
            Write the complete statement, select a word or phrase, then mark it as a
            blank. Each marked selection receives a stable blank number.
          </p>
        </div>
        <span className="count-badge">
          {groups.length} {groups.length === 1 ? "blank" : "blanks"}
        </span>
      </div>

      <label className="cloze-source-field">
        <span>Complete statement</span>
        <textarea
          ref={textareaRef}
          value={source}
          rows={9}
          maxLength={200000}
          disabled={disabled}
          placeholder="ACE inhibitors block the enzyme that creates angiotensin II."
          onChange={(event) => {
            setSelectionError(null);
            onSourceChange(event.currentTarget.value);
          }}
        />
      </label>

      <div className="cloze-toolbar">
        <button
          type="button"
          className="secondary-button"
          disabled={disabled}
          onClick={markSelection}
        >
          Mark selection as blank
        </button>
        <button
          type="button"
          className="text-button"
          disabled={disabled || groups.length === 0}
          onClick={() => {
            setSelectionError(null);
            onSourceChange(removeAllClozeMarkers(source));
          }}
        >
          Remove all blank markers
        </button>
      </div>
      {selectionError ? <p className="field-error">{selectionError}</p> : null}

      <fieldset className="cloze-mode-options" disabled={disabled}>
        <legend>Cards generated from multiple blanks</legend>
        <label>
          <input
            type="radio"
            name="cloze-mode"
            checked={mode === "one_at_a_time"}
            onChange={() => onModeChange("one_at_a_time")}
          />
          <span>
            <strong>Hide one blank at a time</strong>
            <small>
              Generate one independently scheduled card for each numbered blank.
            </small>
          </span>
        </label>
        <label>
          <input
            type="radio"
            name="cloze-mode"
            checked={mode === "all_together"}
            onChange={() => onModeChange("all_together")}
          />
          <span>
            <strong>Hide all blanks together</strong>
            <small>Generate one card with every marked word or phrase hidden.</small>
          </span>
        </label>
      </fieldset>

      <div className="cloze-preview-panel">
        <div className="preview-panel-heading">
          <div>
            <span className="eyebrow">PREVIEW</span>
            <h3>Generated card</h3>
          </div>
          {mode === "one_at_a_time" && groups.length > 1 ? (
            <label>
              <span>Blank</span>
              <select
                value={effectivePreviewGroup ?? ""}
                onChange={(event) => setPreviewGroup(Number(event.currentTarget.value))}
              >
                {groups.map((group) => (
                  <option key={group} value={group}>
                    Blank {group}
                  </option>
                ))}
              </select>
            </label>
          ) : null}
        </div>
        <div className="cloze-preview-sides">
          <div>
            <small>Question</small>
            <p>{preview.question || "The question preview appears here."}</p>
          </div>
          <div>
            <small>Answer</small>
            <p>{preview.answer}</p>
          </div>
        </div>
      </div>
    </section>
  );
}

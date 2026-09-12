import { Surface } from "../../ui/primitives/Surface";
import { Badge } from "../../ui/primitives/Badge";
import { RadioGroup } from "../../ui/primitives/FormControls";
import { presentation } from "./presentation";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { Textarea } from "../../ui/primitives/FormControls";
import { useMemo, useRef, useState } from "react";
import { FieldError } from "../../ui/primitives/FormField";

import {
  type ClozeMode,
} from "../../ipc/cards";
import {
  distinctClozeGroups,
  markSelectionAsBlank,
  removeAllClozeMarkers,
  renderClozePreview,
} from "./cloze";
import { Button } from "../../ui/primitives/Button";
import { Panel } from "../../ui/primitives/Panel";

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
  const countLabel =
    mode === "all_together"
      ? "1 card · all blanks"
      : `1 card · ${groups.length} alternating ${groups.length === 1 ? "blank" : "blanks"}`;

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
    <Surface className={presentation["cloze-editor"]} aria-labelledby="cloze-editor-heading">
      <div className={presentation["section-heading"]}>
        <div>
          <h2 id="cloze-editor-heading">Fill in the blank</h2>
          <p>
            Enter the complete statement, select a word or phrase, then mark it as a
            blank. Multiple marked blanks remain alternate presentations of one
            scheduled card.
          </p>
        </div>
        <Badge tone="accent">{countLabel}</Badge>
      </div>

      <label className={presentation["cloze-source-field"]}>
        <span>Complete statement</span>
        <Textarea
          ref={textareaRef}
          value={source}
          rows={9}
          maxLength={200000}
          disabled={disabled}
          placeholder="Enter the complete statement, then select text to mark as a blank."
          onChange={(event) => {
            setSelectionError(null);
            onSourceChange(event.currentTarget.value);
          }}
        />
      </label>

      <div className={presentation["cloze-toolbar"]}>
        <Button
          type="button"
          variant="secondary"
          disabled={disabled}
          onClick={markSelection}
        >
          Mark selection as blank
        </Button>
        <Button
          type="button"
          variant="text"
          disabled={disabled || groups.length === 0}
          onClick={() => {
            setSelectionError(null);
            onSourceChange(removeAllClozeMarkers(source));
          }}
        >
          Remove all blank markers
        </Button>
      </div>
      {selectionError ? <FieldError>{selectionError}</FieldError> : null}

      <div className="tw:grid tw:gap-3">
        <h3 className="tw:m-0 tw:text-label tw:font-medium">Blank behavior</h3>
        <RadioGroup label="Blank behavior" name="cloze-mode" value={mode} disabled={disabled} onValueChange={onModeChange}
          options={[
            { value: "one_at_a_time", label: <span className="tw:grid tw:gap-1"><strong>Hide one blank per review</strong><small className="tw:font-normal tw:text-muted-foreground">One scheduled card rotates through the marked blanks over successive reviews.</small></span> },
            { value: "all_together", label: <span className="tw:grid tw:gap-1"><strong>Hide all blanks together</strong><small className="tw:font-normal tw:text-muted-foreground">Use one scheduled card with every marked word or phrase hidden.</small></span> },
          ]} />
      </div>

      <Panel as="div">
        <div className={presentation["preview-panel-heading"]}>
          <div>
            <span className={presentation["eyebrow"]}>PREVIEW</span>
            <h3>Card presentation</h3>
          </div>
          {mode === "one_at_a_time" && groups.length > 1 ? (
            <label>
              <span>Preview blank</span>
              <NativeSelect
                value={effectivePreviewGroup ?? ""}
                onChange={(event) => setPreviewGroup(Number(event.currentTarget.value))}
              >
                {groups.map((group) => (
                  <option key={group} value={group}>
                    Blank {group}
                  </option>
                ))}
              </NativeSelect>
            </label>
          ) : null}
        </div>
        <div className={presentation["cloze-preview-sides"]}>
          <div>
            <small>Question</small>
            <p>{preview.question || "The question preview appears here."}</p>
          </div>
          <div>
            <small>Answer</small>
            <p>{preview.answer}</p>
          </div>
        </div>
      </Panel>
    </Surface>
  );
}

import type { Asset } from "../../ipc/assets";
import { RichFieldList } from "../../editor/RichFieldList";
import { Button } from "../../ui/primitives/Button";
import { Checkbox, RadioGroup } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Surface } from "../../ui/primitives/Surface";
import {
  addChoiceOption,
  moveChoiceOption,
  removeChoiceOption,
  updateChoiceOption,
  type QuestionEditorDraft,
} from "./questionDraft";

type ChoiceDraft = Extract<QuestionEditorDraft, { kind: "single_choice" | "select_all" }>;

interface ChoiceQuestionEditorProps {
  draft: ChoiceDraft;
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onAssetImported: (asset: Asset) => void;
  onChange: (draft: ChoiceDraft) => void;
}

export function ChoiceQuestionEditor({
  draft,
  assets,
  assetDraftId,
  disabled,
  onAssetImported,
  onChange,
}: ChoiceQuestionEditorProps) {
  const title = draft.kind === "single_choice" ? "Single-best-answer" : "Select all that apply";
  return (
    <Surface className="tw:grid tw:gap-5 tw:p-5">
      <div className="tw:grid tw:gap-1">
        <h2>{title}</h2>
        <p className="tw:text-sm tw:text-muted-foreground">
          {draft.kind === "single_choice"
            ? "Write one defensible best answer. Distractors may be plausible, but the key must contain exactly one option."
            : "Learners must choose the exact correct set. Option rationales appear only after Check answer."}
        </p>
      </div>

      <RichFieldList
        fields={[
          { id: "prompt", label: "Question", document: draft.prompt, minHeight: 120 },
          {
            id: "explanation",
            label: "Overall explanation (optional)",
            description: "Shown after the first answer is checked.",
            document: draft.explanation,
            minHeight: 90,
          },
        ]}
        assets={assets}
        assetDraftId={assetDraftId}
        disabled={disabled}
        onAssetImported={onAssetImported}
        onFieldChange={(fieldId, document, text) => {
          onChange(fieldId === "prompt"
            ? { ...draft, prompt: document, promptText: text }
            : { ...draft, explanation: document, explanationText: text });
        }}
      />

      {draft.kind === "single_choice" ? (
        <RadioGroup
          label="Correct answer"
          value={draft.correctOptionIds[0] ?? ""}
          options={draft.options.map((option, index) => ({
            value: option.id,
            label: `Option ${index + 1}`,
          }))}
          disabled={disabled}
          onValueChange={(optionId) => onChange({
            ...draft,
            correctOptionIds: [optionId],
          })}
        />
      ) : (
        <div className="tw:grid tw:gap-2">
          <strong>Correct options</strong>
          <p className="tw:text-sm tw:text-muted-foreground">
            Check every option that belongs in the exact answer set.
          </p>
          <label className="tw:flex tw:min-h-11 tw:items-center tw:gap-3">
            <Checkbox
              checked={draft.emptySetConfirmed}
              disabled={disabled}
              onCheckedChange={(checked) => onChange({
                ...draft,
                emptySetConfirmed: checked,
                correctOptionIds: checked ? [] : draft.correctOptionIds,
              })}
            />
            <span>No options are correct</span>
          </label>
        </div>
      )}

      <div className="tw:grid tw:gap-4">
        {draft.options.map((option, index) => (
          <Surface key={option.id} className="tw:grid tw:gap-4 tw:p-4">
            <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
              <strong>Option {index + 1}</strong>
              <div className="tw:flex tw:flex-wrap tw:gap-2">
                {draft.kind === "select_all" ? (
                  <label className="tw:flex tw:min-h-11 tw:items-center tw:gap-2">
                    <Checkbox
                      checked={draft.correctOptionIds.includes(option.id)}
                      disabled={disabled || draft.emptySetConfirmed}
                      onCheckedChange={(checked) => onChange({
                        ...draft,
                        emptySetConfirmed: false,
                        correctOptionIds: checked
                          ? [...draft.correctOptionIds, option.id]
                          : draft.correctOptionIds.filter((id) => id !== option.id),
                      })}
                    />
                    <span>Correct</span>
                  </label>
                ) : null}
                <Button
                  variant="secondary"
                  size="small"
                  disabled={disabled || index === 0}
                  onClick={() => onChange(moveChoiceOption(draft, option.id, -1) as ChoiceDraft)}
                >
                  Move up
                </Button>
                <Button
                  variant="secondary"
                  size="small"
                  disabled={disabled || index === draft.options.length - 1}
                  onClick={() => onChange(moveChoiceOption(draft, option.id, 1) as ChoiceDraft)}
                >
                  Move down
                </Button>
                <Button
                  variant="danger"
                  size="small"
                  disabled={disabled || draft.options.length <= 2}
                  onClick={() => onChange(removeChoiceOption(draft, option.id) as ChoiceDraft)}
                >
                  Remove
                </Button>
              </div>
            </div>
            <RichFieldList
              fields={[
                { id: `option:${option.id}`, label: "Answer option", document: option.doc, minHeight: 85 },
                {
                  id: `rationale:${option.id}`,
                  label: "Why this option is or is not correct (optional)",
                  document: option.rationale,
                  minHeight: 75,
                },
              ]}
              assets={assets}
              assetDraftId={assetDraftId}
              disabled={disabled}
              onAssetImported={onAssetImported}
              onFieldChange={(fieldId, document, text) => {
                const changed = fieldId.startsWith("rationale:")
                  ? updateChoiceOption(draft, option.id, { rationale: document, rationaleText: text })
                  : updateChoiceOption(draft, option.id, { doc: document, text });
                onChange(changed as ChoiceDraft);
              }}
            />
          </Surface>
        ))}
      </div>

      {draft.kind === "select_all" && draft.correctOptionIds.length === 0 && !draft.emptySetConfirmed ? (
        <InlineNotice kind="info">
          If the intended key is an empty set, explicitly choose “No options are correct.” This keeps an
          unfinished question distinct from a deliberately empty answer.
        </InlineNotice>
      ) : null}

      <div>
        <Button
          variant="secondary"
          disabled={disabled || draft.options.length >= 32}
          onClick={() => onChange(addChoiceOption(draft) as ChoiceDraft)}
        >
          Add option
        </Button>
      </div>
    </Surface>
  );
}

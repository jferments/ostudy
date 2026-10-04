import type { Asset } from "../../ipc/assets";
import { RichFieldList } from "../../editor/RichFieldList";
import { Button } from "../../ui/primitives/Button";
import { Checkbox } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Surface } from "../../ui/primitives/Surface";
import { AnswerChoices } from "../practice/AnswerChoices";
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
  const previewOptions = draft.options.map((option) => ({ id: option.id, content: option.doc }));
  return (
    <Surface className="tw:grid tw:gap-5 tw:p-5">
      <div className="tw:grid tw:gap-1">
        <h2>{title}</h2>
        <p className="tw:text-small tw:text-muted-foreground">
          {draft.kind === "single_choice"
            ? "Write one defensible best answer. Distractors may be plausible, but the key must contain exactly one option."
            : "Learners must choose the exact correct set. Option explanations appear only after Check answer."}
        </p>
      </div>

      <RichFieldList
        fields={[
          { id: "prompt", label: "Question", document: draft.prompt, minHeight: 120 },
          {
            id: "explanation",
            label: "Overall explanation",
            description: "Shown after the first answer is checked.",
            document: draft.explanation,
            minHeight: 90,
            optional: true,
            optionalLabel: "overall explanation",
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

      <div className="tw:grid tw:gap-4">
        {draft.options.map((option, index) => (
          <Surface key={option.id} className="tw:grid tw:gap-4 tw:p-4">
            <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
              <strong>Option {index + 1}</strong>
              <div className="tw:flex tw:flex-wrap tw:gap-2">
                <Button variant="secondary" size="small" disabled={disabled || index === 0}
                  onClick={() => onChange(moveChoiceOption(draft, option.id, -1) as ChoiceDraft)}>
                  Move up
                </Button>
                <Button variant="secondary" size="small" disabled={disabled || index === draft.options.length - 1}
                  onClick={() => onChange(moveChoiceOption(draft, option.id, 1) as ChoiceDraft)}>
                  Move down
                </Button>
                <Button variant="danger" size="small" disabled={disabled || draft.options.length <= 2}
                  onClick={() => onChange(removeChoiceOption(draft, option.id) as ChoiceDraft)}>
                  Remove
                </Button>
              </div>
            </div>
            <RichFieldList
              fields={[
                { id: `option:${option.id}`, label: "Answer option", document: option.doc, minHeight: 85 },
                {
                  id: `rationale:${option.id}`,
                  label: "Option explanation",
                  description: "Explain why this option is correct or incorrect after the learner checks their answer.",
                  document: option.rationale,
                  minHeight: 75,
                  optional: true,
                  optionalLabel: "option explanation",
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

      <section className="tw:grid tw:gap-3 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-muted/20 tw:p-4"
        aria-label="Answer key preview">
        <div className="tw:grid tw:gap-1">
          <h3 className="tw:m-0">Answer key preview</h3>
          <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
            {draft.kind === "single_choice" ? "Select the correct answer below." : "Select every correct answer below."}
            {" "}This preview uses the same option tiles as Practice.
          </p>
        </div>
        <AnswerChoices options={previewOptions} selectedIds={draft.correctOptionIds}
          multiple={draft.kind === "select_all"} assets={assets} disabled={disabled}
          label={draft.kind === "single_choice" ? "Correct answer" : "Correct answers"}
          onError={() => undefined}
          onChange={(ids) => onChange({ ...draft, correctOptionIds: ids, emptySetConfirmed: false })} />
        {draft.kind === "select_all" ? <label className="tw:flex tw:min-h-11 tw:items-center tw:gap-3">
          <Checkbox checked={draft.emptySetConfirmed} disabled={disabled}
            onCheckedChange={(checked) => onChange({
              ...draft,
              emptySetConfirmed: checked,
              correctOptionIds: checked ? [] : draft.correctOptionIds,
            })} />
          <span>No options are correct</span>
        </label> : null}
      </section>

      {draft.kind === "select_all" && draft.correctOptionIds.length === 0 && !draft.emptySetConfirmed ? (
        <InlineNotice kind="info">
          If the intended key is an empty set, explicitly choose “No options are correct.” This keeps an
          unfinished question distinct from a deliberately empty answer.
        </InlineNotice>
      ) : null}

      <div>
        <Button variant="secondary" disabled={disabled || draft.options.length >= 32}
          onClick={() => onChange(addChoiceOption(draft) as ChoiceDraft)}>
          Add option
        </Button>
      </div>
    </Surface>
  );
}

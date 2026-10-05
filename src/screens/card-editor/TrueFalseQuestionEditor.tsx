import type { Asset } from "../../ipc/assets";
import { RichFieldList } from "../../editor/RichFieldList";
import { RadioGroup } from "../../ui/primitives/FormControls";
import { Surface } from "../../ui/primitives/Surface";
import type { QuestionEditorDraft } from "./questionDraft";

type TrueFalseDraft = Extract<QuestionEditorDraft, { kind: "true_false" }>;

interface TrueFalseQuestionEditorProps {
  draft: TrueFalseDraft;
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onAssetImported: (asset: Asset) => void;
  onChange: (draft: TrueFalseDraft) => void;
}

export function TrueFalseQuestionEditor({
  draft,
  assets,
  assetDraftId,
  disabled,
  onAssetImported,
  onChange,
}: TrueFalseQuestionEditorProps) {
  return (
    <Surface className="tw:grid tw:gap-5 tw:p-5">
      <div className="tw:grid tw:gap-1">
        <h2>True / false</h2>
        <p className="tw:text-small tw:text-muted-foreground">
          Keep the statement atomic. Avoid compound claims, hidden qualifiers, and double negatives.
        </p>
      </div>
      <RichFieldList
        fields={[
          { id: "statement", label: "Statement", document: draft.statement, minHeight: 120 },
          {
            id: "explanation",
            label: "Explanation",
            description: "Shown after Check answer.",
            document: draft.explanation,
            minHeight: 90,
            optional: true,
            optionalLabel: "explanation",
          },
          {
            id: "corrected",
            label: "Corrected statement",
            description: "Useful when a false statement should be repaired rather than merely rejected.",
            document: draft.correctedStatement,
            minHeight: 90,
            optional: true,
            optionalLabel: "corrected statement",
          },
        ]}
        assets={assets}
        assetDraftId={assetDraftId}
        disabled={disabled}
        onAssetImported={onAssetImported}
        onFieldChange={(fieldId, document, text) => {
          if (fieldId === "statement") onChange({ ...draft, statement: document, statementText: text });
          else if (fieldId === "explanation") onChange({ ...draft, explanation: document, explanationText: text });
          else onChange({ ...draft, correctedStatement: document, correctedStatementText: text });
        }}
      />
      <RadioGroup
        label="Correct answer"
        value={draft.correct === null ? "" : draft.correct ? "true" : "false"}
        options={[
          { value: "true", label: "True" },
          { value: "false", label: "False" },
        ]}
        disabled={disabled}
        presentation="cards"
        onValueChange={(value) => onChange({ ...draft, correct: value === "true" })}
      />
    </Surface>
  );
}

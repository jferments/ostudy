import type { Asset } from "../../ipc/assets";
import { RichFieldList } from "../../editor/RichFieldList";
import { Button } from "../../ui/primitives/Button";
import { Surface } from "../../ui/primitives/Surface";
import {
  addMatchingPair,
  moveMatchingPair,
  removeMatchingPair,
  type QuestionEditorDraft,
} from "./questionDraft";

type MatchingDraft = Extract<QuestionEditorDraft, { kind: "matching" }>;

interface MatchingQuestionEditorProps {
  draft: MatchingDraft;
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onAssetImported: (asset: Asset) => void;
  onChange: (draft: MatchingDraft) => void;
}

export function MatchingQuestionEditor({
  draft,
  assets,
  assetDraftId,
  disabled,
  onAssetImported,
  onChange,
}: MatchingQuestionEditorProps) {
  return (
    <Surface className="tw:grid tw:gap-5 tw:p-5">
      <div className="tw:grid tw:gap-1">
        <h2>Matching</h2>
        <p className="tw:text-sm tw:text-muted-foreground">
          Author explicit one-to-one pairs. Learners see the two sides shuffled independently.
        </p>
      </div>
      <RichFieldList
        fields={[
          { id: "prompt", label: "Instructions", document: draft.prompt, minHeight: 100 },
          {
            id: "explanation",
            label: "Overall explanation",
            description: "Shown after the first submitted arrangement is checked.",
            document: draft.explanation,
            minHeight: 80,
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
        {draft.pairs.map((pair, index) => (
          <Surface key={pair.id} className="tw:grid tw:gap-4 tw:p-4">
            <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
              <strong>Pair {index + 1}</strong>
              <div className="tw:flex tw:flex-wrap tw:gap-2">
                <Button variant="secondary" size="small" disabled={disabled || index === 0}
                  onClick={() => onChange(moveMatchingPair(draft, pair.id, -1) as MatchingDraft)}>
                  Move up
                </Button>
                <Button variant="secondary" size="small"
                  disabled={disabled || index === draft.pairs.length - 1}
                  onClick={() => onChange(moveMatchingPair(draft, pair.id, 1) as MatchingDraft)}>
                  Move down
                </Button>
                <Button variant="danger" size="small" disabled={disabled || draft.pairs.length <= 2}
                  onClick={() => onChange(removeMatchingPair(draft, pair.id) as MatchingDraft)}>
                  Remove
                </Button>
              </div>
            </div>
            <RichFieldList
              fields={[
                { id: `left:${pair.id}`, label: "Left item", document: pair.left.doc, minHeight: 80 },
                { id: `right:${pair.id}`, label: "Matching right item", document: pair.right.doc, minHeight: 80 },
                {
                  id: `rationale:${pair.id}`,
                  label: "Pair explanation",
                  description: "Explain why these two items belong together after checking.",
                  document: pair.rationale,
                  minHeight: 70,
                  optional: true,
                  optionalLabel: "pair explanation",
                },
              ]}
              assets={assets}
              assetDraftId={assetDraftId}
              disabled={disabled}
              onAssetImported={onAssetImported}
              onFieldChange={(fieldId, document, text) => {
                const pairs = draft.pairs.map((candidate) => {
                  if (candidate.id !== pair.id) return candidate;
                  if (fieldId.startsWith("left:")) return {
                    ...candidate,
                    left: { ...candidate.left, doc: document, text },
                  };
                  if (fieldId.startsWith("right:")) return {
                    ...candidate,
                    right: { ...candidate.right, doc: document, text },
                  };
                  return { ...candidate, rationale: document, rationaleText: text };
                });
                onChange({ ...draft, pairs });
              }}
            />
          </Surface>
        ))}
      </div>
      <div>
        <Button variant="secondary" disabled={disabled || draft.pairs.length >= 16}
          onClick={() => onChange(addMatchingPair(draft) as MatchingDraft)}>
          Add pair
        </Button>
      </div>
    </Surface>
  );
}

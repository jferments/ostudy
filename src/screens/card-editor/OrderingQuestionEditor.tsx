import { RichFieldList } from "../../editor/RichFieldList";
import type { Asset } from "../../ipc/assets";
import { Button } from "../../ui/primitives/Button";
import { Surface } from "../../ui/primitives/Surface";
import {
  addOrderingItem,
  moveOrderingItem,
  removeOrderingItem,
  type QuestionEditorDraft,
} from "./questionDraft";

type OrderingDraft = Extract<QuestionEditorDraft, { kind: "ordering" }>;

interface OrderingQuestionEditorProps {
  draft: OrderingDraft;
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onAssetImported: (asset: Asset) => void;
  onChange: (draft: OrderingDraft) => void;
}

export function OrderingQuestionEditor({
  draft,
  assets,
  assetDraftId,
  disabled,
  onAssetImported,
  onChange,
}: OrderingQuestionEditorProps) {
  return (
    <Surface className="tw:grid tw:gap-5 tw:p-5">
      <div className="tw:grid tw:gap-1">
        <h2>Ordering</h2>
        <p className="tw:text-sm tw:text-muted-foreground">
          State the ordering criterion clearly, then enter the items below in the one accepted correct order.
          Practice shuffles them before the learner answers.
        </p>
      </div>
      <RichFieldList
        fields={[
          { id: "prompt", label: "Ordering criterion / instructions", document: draft.prompt, minHeight: 100 },
          {
            id: "explanation",
            label: "Overall explanation",
            description: "Shown after the submitted order is checked.",
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
      <div className="tw:grid tw:gap-4" aria-label="Correct item order">
        {draft.items.map((item, index) => (
          <Surface key={item.id} className="tw:grid tw:gap-4 tw:p-4">
            <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
              <strong>Position {index + 1}</strong>
              <div className="tw:flex tw:flex-wrap tw:gap-2">
                <Button variant="secondary" size="small" disabled={disabled || index === 0}
                  onClick={() => onChange(moveOrderingItem(draft, item.id, -1) as OrderingDraft)}>
                  Move up
                </Button>
                <Button variant="secondary" size="small" disabled={disabled || index === draft.items.length - 1}
                  onClick={() => onChange(moveOrderingItem(draft, item.id, 1) as OrderingDraft)}>
                  Move down
                </Button>
                <Button variant="danger" size="small" disabled={disabled || draft.items.length <= 2}
                  onClick={() => onChange(removeOrderingItem(draft, item.id) as OrderingDraft)}>
                  Remove
                </Button>
              </div>
            </div>
            <RichFieldList
              fields={[{ id: `order:${item.id}`, label: `Item ${index + 1}`, document: item.doc, minHeight: 80 }]}
              assets={assets}
              assetDraftId={assetDraftId}
              disabled={disabled}
              onAssetImported={onAssetImported}
              onFieldChange={(_fieldId, document, text) => onChange({
                ...draft,
                items: draft.items.map((candidate) => candidate.id === item.id
                  ? { ...candidate, doc: document, text }
                  : candidate),
              })}
            />
          </Surface>
        ))}
      </div>
      <div>
        <Button variant="secondary" disabled={disabled || draft.items.length >= 32}
          onClick={() => onChange(addOrderingItem(draft) as OrderingDraft)}>
          Add item
        </Button>
      </div>
    </Surface>
  );
}

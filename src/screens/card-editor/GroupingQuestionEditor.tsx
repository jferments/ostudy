import { RichFieldList } from "../../editor/RichFieldList";
import type { Asset } from "../../ipc/assets";
import { Button } from "../../ui/primitives/Button";
import { FormField } from "../../ui/primitives/FormField";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { Surface } from "../../ui/primitives/Surface";
import {
  addGroupingCategory,
  addGroupingItem,
  moveGroupingCategory,
  moveGroupingItem,
  removeGroupingCategory,
  removeGroupingItem,
  type QuestionEditorDraft,
} from "./questionDraft";

type GroupingDraft = Extract<QuestionEditorDraft, { kind: "grouping" }>;

interface GroupingQuestionEditorProps {
  draft: GroupingDraft;
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onAssetImported: (asset: Asset) => void;
  onChange: (draft: GroupingDraft) => void;
}

export function GroupingQuestionEditor({
  draft,
  assets,
  assetDraftId,
  disabled,
  onAssetImported,
  onChange,
}: GroupingQuestionEditorProps) {
  return (
    <Surface className="tw:grid tw:gap-6 tw:p-5">
      <div className="tw:grid tw:gap-1">
        <h2>Grouping</h2>
        <p className="tw:text-sm tw:text-muted-foreground">
          Define mutually exclusive categories, then assign every item to exactly one correct category.
        </p>
      </div>

      <RichFieldList
        fields={[
          { id: "prompt", label: "Instructions / grouping criterion", document: draft.prompt, minHeight: 100 },
          {
            id: "explanation",
            label: "Overall explanation",
            description: "Shown after the first submitted grouping is checked.",
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

      <section className="tw:grid tw:gap-4" aria-labelledby="grouping-categories-heading">
        <div className="tw:flex tw:flex-wrap tw:items-end tw:justify-between tw:gap-3">
          <div className="tw:grid tw:gap-1">
            <h3 id="grouping-categories-heading">Categories</h3>
            <p className="tw:text-sm tw:text-muted-foreground">Create 2–8 distinct destination groups.</p>
          </div>
          <Button variant="secondary" disabled={disabled || draft.categories.length >= 8}
            onClick={() => onChange(addGroupingCategory(draft) as GroupingDraft)}>
            Add category
          </Button>
        </div>
        {draft.categories.map((category, index) => (
          <Surface key={category.id} className="tw:grid tw:gap-3 tw:p-4">
            <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
              <strong>Category {index + 1}</strong>
              <div className="tw:flex tw:flex-wrap tw:gap-2">
                <Button variant="secondary" size="small" disabled={disabled || index === 0}
                  onClick={() => onChange(moveGroupingCategory(draft, category.id, -1) as GroupingDraft)}>
                  Move up
                </Button>
                <Button variant="secondary" size="small" disabled={disabled || index === draft.categories.length - 1}
                  onClick={() => onChange(moveGroupingCategory(draft, category.id, 1) as GroupingDraft)}>
                  Move down
                </Button>
                <Button variant="danger" size="small" disabled={disabled || draft.categories.length <= 2}
                  onClick={() => onChange(removeGroupingCategory(draft, category.id) as GroupingDraft)}>
                  Remove
                </Button>
              </div>
            </div>
            <RichFieldList
              fields={[{ id: `category:${category.id}`, label: "Category name / definition", document: category.doc, minHeight: 75 }]}
              assets={assets}
              assetDraftId={assetDraftId}
              disabled={disabled}
              onAssetImported={onAssetImported}
              onFieldChange={(_fieldId, document, text) => onChange({
                ...draft,
                categories: draft.categories.map((candidate) => candidate.id === category.id
                  ? { ...candidate, doc: document, text }
                  : candidate),
              })}
            />
          </Surface>
        ))}
      </section>

      <section className="tw:grid tw:gap-4" aria-labelledby="grouping-items-heading">
        <div className="tw:flex tw:flex-wrap tw:items-end tw:justify-between tw:gap-3">
          <div className="tw:grid tw:gap-1">
            <h3 id="grouping-items-heading">Items</h3>
            <p className="tw:text-sm tw:text-muted-foreground">Each item has one authored correct category.</p>
          </div>
          <Button variant="secondary" disabled={disabled || draft.items.length >= 32}
            onClick={() => onChange(addGroupingItem(draft) as GroupingDraft)}>
            Add item
          </Button>
        </div>
        {draft.items.map((item, index) => (
          <Surface key={item.id} className="tw:grid tw:gap-4 tw:p-4">
            <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
              <strong>Item {index + 1}</strong>
              <div className="tw:flex tw:flex-wrap tw:gap-2">
                <Button variant="secondary" size="small" disabled={disabled || index === 0}
                  onClick={() => onChange(moveGroupingItem(draft, item.id, -1) as GroupingDraft)}>
                  Move up
                </Button>
                <Button variant="secondary" size="small" disabled={disabled || index === draft.items.length - 1}
                  onClick={() => onChange(moveGroupingItem(draft, item.id, 1) as GroupingDraft)}>
                  Move down
                </Button>
                <Button variant="danger" size="small" disabled={disabled || draft.items.length <= 2}
                  onClick={() => onChange(removeGroupingItem(draft, item.id) as GroupingDraft)}>
                  Remove
                </Button>
              </div>
            </div>
            <RichFieldList
              fields={[
                { id: `item:${item.id}`, label: "Item", document: item.doc, minHeight: 80 },
                {
                  id: `rationale:${item.id}`,
                  label: "Item explanation",
                  description: "Explain why this item belongs in its category after checking.",
                  document: item.rationale,
                  minHeight: 70,
                  optional: true,
                  optionalLabel: "item explanation",
                },
              ]}
              assets={assets}
              assetDraftId={assetDraftId}
              disabled={disabled}
              onAssetImported={onAssetImported}
              onFieldChange={(fieldId, document, text) => onChange({
                ...draft,
                items: draft.items.map((candidate) => {
                  if (candidate.id !== item.id) return candidate;
                  return fieldId.startsWith("rationale:")
                    ? { ...candidate, rationale: document, rationaleText: text }
                    : { ...candidate, doc: document, text };
                }),
              })}
            />
            <FormField label="Correct category" description="Choose the single category this item belongs to.">
              <NativeSelect value={item.categoryId} disabled={disabled} onChange={(event) => onChange({
                ...draft,
                items: draft.items.map((candidate) => candidate.id === item.id
                  ? { ...candidate, categoryId: event.currentTarget.value }
                  : candidate),
              })}>
                <option value="">Choose category…</option>
                {draft.categories.map((category, categoryIndex) => (
                  <option key={category.id} value={category.id}>{categoryLabel(category.text, categoryIndex)}</option>
                ))}
              </NativeSelect>
            </FormField>
          </Surface>
        ))}
      </section>
    </Surface>
  );
}

function categoryLabel(text: string, index: number): string {
  const normalized = text.replace(/\s+/gu, " ").trim();
  if (normalized === "") return `Category ${index + 1}`;
  return normalized.length > 70 ? `${normalized.slice(0, 67)}…` : normalized;
}

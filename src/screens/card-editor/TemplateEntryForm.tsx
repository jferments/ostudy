import { Surface } from "../../ui/primitives/Surface";
import { Badge } from "../../ui/primitives/Badge";
import { presentation } from "./presentation";
import { RichTextEditor } from "../../editor/RichTextEditor";
import { RichTextView } from "../../editor/RichTextView";
import {
  type Asset,
} from "../../ipc/assets";
import {
  type RichDocument,
} from "../../ipc/common";
import {
  type CardTemplate,
  type TemplateCardPreview,
  type TemplateValueInput,
} from "../../ipc/templates";
import { Button } from "../../ui/primitives/Button";

interface TemplateEntryFormProps {
  template: CardTemplate;
  values: TemplateValueInput[];
  previews: TemplateCardPreview[];
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  isPreviewing: boolean;
  onValuesChange: (values: TemplateValueInput[]) => void;
  onAssetImported: (asset: Asset) => void;
  onPreview: () => void;
}

export function TemplateEntryForm({
  template,
  values,
  previews,
  assets,
  assetDraftId,
  disabled,
  isPreviewing,
  onValuesChange,
  onAssetImported,
  onPreview,
}: TemplateEntryFormProps) {
  function updateValue(
    fieldId: string,
    doc: RichDocument,
    text: string,
  ): void {
    onValuesChange(
      values.map((entry) =>
        entry.fieldId === fieldId ? { ...entry, doc, text } : entry,
      ),
    );
  }

  return (
    <Surface className={presentation["template-entry-form"]} aria-labelledby="template-entry-heading">
      <div className={presentation["section-heading"]}>
        <div>
          <h2 id="template-entry-heading">{template.name}</h2>
          {template.description ? <p>{template.description}</p> : null}
        </div>
        <Badge tone="accent">
          {template.rules.length} {template.rules.length === 1 ? "card" : "cards"}
        </Badge>
      </div>

      <div className={presentation["template-rich-value-list"]}>
        {template.fields.map((field) => {
          const value = values.find((entry) => entry.fieldId === field.id);
          if (!value) return null;
          return (
            <RichTextEditor
              key={field.id}
              label={field.name}
              description={field.description || "Enter the content for this field."}
              document={value.doc}
              assets={assets}
              assetDraftId={assetDraftId}
              disabled={disabled}
              onAssetImported={onAssetImported}
              onChange={(doc, text) => updateValue(field.id, doc, text)}
              minHeight={180}
            />
          );
        })}
      </div>

      <div className={presentation["template-preview-actions"]}>
        <Button
          type="button"
          variant="secondary"
          disabled={disabled}
          onClick={onPreview}
        >
          {isPreviewing ? "Building preview…" : "Preview generated cards"}
        </Button>
      </div>

      {previews.length > 0 ? (
        <div className={presentation["template-card-previews"]} aria-label="Generated card preview">
          {previews.map((preview) => (
            <article key={preview.ruleId}>
              <span className={presentation["eyebrow"]}>{preview.ruleName}</span>
              <div>
                <small>Question</small>
                <RichTextView
                  document={preview.questionDoc}
                  assets={preview.assets}
                />
              </div>
              <div>
                <small>Answer</small>
                <RichTextView
                  document={preview.answerDoc}
                  assets={preview.assets}
                />
              </div>
            </article>
          ))}
        </div>
      ) : null}
    </Surface>
  );
}

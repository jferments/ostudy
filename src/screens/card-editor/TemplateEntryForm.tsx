import type {
  CardTemplate,
  TemplateCardPreview,
  TemplateValueInput,
} from "../../ipc";

interface TemplateEntryFormProps {
  template: CardTemplate;
  values: TemplateValueInput[];
  previews: TemplateCardPreview[];
  disabled: boolean;
  isPreviewing: boolean;
  onValuesChange: (values: TemplateValueInput[]) => void;
  onPreview: () => void;
}

export function TemplateEntryForm({
  template,
  values,
  previews,
  disabled,
  isPreviewing,
  onValuesChange,
  onPreview,
}: TemplateEntryFormProps) {
  function updateValue(fieldId: string, value: string): void {
    onValuesChange(
      values.map((entry) => (entry.fieldId === fieldId ? { ...entry, value } : entry)),
    );
  }

  return (
    <section className="template-entry-form" aria-labelledby="template-entry-heading">
      <div className="section-heading compact-section-heading">
        <div>
          <h2 id="template-entry-heading">{template.name}</h2>
          {template.description ? <p>{template.description}</p> : null}
        </div>
        <span className="count-badge">
          {template.rules.length} {template.rules.length === 1 ? "card" : "cards"}
        </span>
      </div>

      <div className="template-value-grid">
        {template.fields.map((field) => {
          const value = values.find((entry) => entry.fieldId === field.id)?.value ?? "";
          return (
            <label key={field.id}>
              <span>{field.name}</span>
              <textarea
                value={value}
                disabled={disabled}
                rows={field.description ? 4 : 3}
                maxLength={50000}
                placeholder={field.description || `Enter ${field.name}`}
                onChange={(event) => updateValue(field.id, event.currentTarget.value)}
              />
              {field.description ? <small>{field.description}</small> : null}
            </label>
          );
        })}
      </div>

      <div className="template-preview-actions">
        <button
          type="button"
          className="secondary-button"
          disabled={disabled || values.some((entry) => entry.value.trim() === "")}
          onClick={onPreview}
        >
          {isPreviewing ? "Building preview…" : "Preview generated cards"}
        </button>
        <small>
          Generation uses literal field substitution only. The saved results are normal
          Basic cards and remain after this template is deleted.
        </small>
      </div>

      {previews.length > 0 ? (
        <div className="template-card-previews" aria-label="Generated card preview">
          {previews.map((preview) => (
            <article key={preview.ruleId}>
              <span className="eyebrow">{preview.ruleName}</span>
              <div>
                <small>Question</small>
                <p>{preview.question}</p>
              </div>
              <div>
                <small>Answer</small>
                <p>{preview.answer}</p>
              </div>
            </article>
          ))}
        </div>
      ) : null}
    </section>
  );
}

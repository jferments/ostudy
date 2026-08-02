import { useEffect, useMemo, useState } from "react";
import type { FormEvent } from "react";

import {
  createCardTemplate,
  deleteCardTemplate,
  getErrorMessage,
  listCardTemplates,
  updateCardTemplate,
  type CardTemplate,
  type CardTemplateFieldInput,
  type CardTemplateRuleInput,
} from "../ipc";

interface TemplatesScreenProps {
  onDirtyChange: (dirty: boolean) => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

type EditorMode = "create" | "edit";
type PatternSide = "front" | "back";

interface TemplateDraft {
  name: string;
  description: string;
  fields: CardTemplateFieldInput[];
  rules: CardTemplateRuleInput[];
}

interface ActivePattern {
  ruleIndex: number;
  side: PatternSide;
}

const NEW_TEMPLATE: TemplateDraft = {
  name: "",
  description: "",
  fields: [{ id: null, name: "", description: "" }],
  rules: [
    {
      id: null,
      name: "Primary card",
      frontPattern: "",
      backPattern: "",
    },
  ],
};

export function TemplatesScreen({
  onDirtyChange,
  showToast,
}: TemplatesScreenProps) {
  const [templates, setTemplates] = useState<CardTemplate[]>([]);
  const [mode, setMode] = useState<EditorMode | null>(null);
  const [source, setSource] = useState<CardTemplate | null>(null);
  const [draft, setDraft] = useState<TemplateDraft>(() => cloneDraft(NEW_TEMPLATE));
  const [activePattern, setActivePattern] = useState<ActivePattern>({
    ruleIndex: 0,
    side: "front",
  });
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const placeholderNames = useMemo(
    () => draft.fields.map((field) => collapseWhitespace(field.name)).filter(Boolean),
    [draft.fields],
  );

  useEffect(() => {
    let active = true;
    void loadTemplates()
      .then((loaded) => {
        if (active) setTemplates(loaded);
      })
      .catch((error: unknown) => {
        if (active) setErrorMessage(getErrorMessage(error));
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);

  function markDirty(): void {
    setErrorMessage(null);
    onDirtyChange(true);
  }

  function beginCreate(): void {
    setMode("create");
    setSource(null);
    setDraft(cloneDraft(NEW_TEMPLATE));
    setActivePattern({ ruleIndex: 0, side: "front" });
    setErrorMessage(null);
    onDirtyChange(false);
  }

  function beginEdit(template: CardTemplate): void {
    setMode("edit");
    setSource(template);
    setDraft({
      name: template.name,
      description: template.description,
      fields: template.fields.map((field) => ({
        id: field.id,
        name: field.name,
        description: field.description,
      })),
      rules: template.rules.map((rule) => ({
        id: rule.id,
        name: rule.name,
        frontPattern: rule.frontPattern,
        backPattern: rule.backPattern,
      })),
    });
    setActivePattern({ ruleIndex: 0, side: "front" });
    setErrorMessage(null);
    onDirtyChange(false);
  }

  function closeEditor(): void {
    setMode(null);
    setSource(null);
    setErrorMessage(null);
    onDirtyChange(false);
  }

  async function refreshTemplates(): Promise<CardTemplate[]> {
    const loaded = await loadTemplates();
    setTemplates(loaded);
    return loaded;
  }

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (isSaving) return;
    const validation = validateDraft(draft);
    if (validation) {
      setErrorMessage(validation);
      return;
    }

    setIsSaving(true);
    setErrorMessage(null);
    try {
      const saved = source
        ? await updateCardTemplate({
            templateId: source.id,
            revision: source.revision,
            ...draft,
          })
        : await createCardTemplate(draft);
      await refreshTemplates();
      onDirtyChange(false);
      setMode(null);
      setSource(null);
      showToast(
        "success",
        source ? `Updated “${saved.name}”.` : `Created “${saved.name}”.`,
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsSaving(false);
    }
  }

  async function removeTemplate(template: CardTemplate): Promise<void> {
    if (
      !window.confirm(
        `Delete the “${template.name}” card template?\n\nCards previously generated with this template are ordinary deck cards and will remain unchanged.`,
      )
    ) return;
    try {
      await deleteCardTemplate(template.id);
      await refreshTemplates();
      showToast("success", `Deleted “${template.name}”.`);
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  function updateField(
    index: number,
    key: "name" | "description",
    value: string,
  ): void {
    setDraft((current) => ({
      ...current,
      fields: current.fields.map((field, fieldIndex) =>
        fieldIndex === index ? { ...field, [key]: value } : field,
      ),
    }));
    markDirty();
  }

  function updateRule(
    index: number,
    key: "name" | "frontPattern" | "backPattern",
    value: string,
  ): void {
    setDraft((current) => ({
      ...current,
      rules: current.rules.map((rule, ruleIndex) =>
        ruleIndex === index ? { ...rule, [key]: value } : rule,
      ),
    }));
    markDirty();
  }

  function insertPlaceholder(fieldName: string): void {
    const placeholder = `{{${fieldName}}}`;
    const key = activePattern.side === "front" ? "frontPattern" : "backPattern";
    setDraft((current) => ({
      ...current,
      rules: current.rules.map((rule, index) => {
        if (index !== activePattern.ruleIndex) return rule;
        const existing = rule[key];
        const separator = existing === "" || /\s$/u.test(existing) ? "" : " ";
        return { ...rule, [key]: `${existing}${separator}${placeholder}` };
      }),
    }));
    markDirty();
  }

  if (mode) {
    return (
      <form className="screen template-editor-screen" onSubmit={(event) => void submit(event)}>
        <header className="screen-header template-editor-header">
          <div>
            <button type="button" className="back-button" onClick={closeEditor}>
              ← Templates
            </button>
            <h1>{mode === "create" ? "Create card template" : `Edit ${source?.name ?? "template"}`}</h1>
            <p>
              Define data fields and deterministic question/answer rules. Templates substitute
              plain text only; they cannot execute code or render raw HTML.
            </p>
          </div>
          <div className="header-actions wrap-actions">
            <button type="button" className="secondary-button" disabled={isSaving} onClick={closeEditor}>
              Cancel
            </button>
            <button type="submit" className="primary-button" disabled={isSaving}>
              {isSaving ? "Saving…" : "Save template"}
            </button>
          </div>
        </header>

        {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}

        <div className="template-editor-layout">
          <main className="template-editor-main">
            <section className="template-definition-card template-identity-card">
              <label>
                <span>Template name</span>
                <input
                  value={draft.name}
                  disabled={isSaving}
                  maxLength={80}
                  placeholder="Pharmacology"
                  onChange={(event) => {
                    setDraft((current) => ({ ...current, name: event.currentTarget.value }));
                    markDirty();
                  }}
                />
              </label>
              <label>
                <span>Description <small>optional</small></span>
                <textarea
                  value={draft.description}
                  disabled={isSaving}
                  maxLength={2000}
                  rows={3}
                  placeholder="Structured medication facts that generate several study cards."
                  onChange={(event) => {
                    setDraft((current) => ({
                      ...current,
                      description: event.currentTarget.value,
                    }));
                    markDirty();
                  }}
                />
              </label>
            </section>

            <section className="template-definition-card">
              <header className="template-section-heading">
                <div>
                  <span className="section-kicker">1 · Source data</span>
                  <h2>Fields</h2>
                  <p>Each field becomes one input in the template generation form.</p>
                </div>
                <button
                  type="button"
                  className="secondary-button"
                  disabled={isSaving || draft.fields.length >= 32}
                  onClick={() => {
                    setDraft((current) => ({
                      ...current,
                      fields: [...current.fields, { id: null, name: "", description: "" }],
                    }));
                    markDirty();
                  }}
                >
                  Add field
                </button>
              </header>

              <div className="template-field-list">
                {draft.fields.map((field, index) => (
                  <article className="template-field-row" key={field.id ?? `new-field-${index}`}>
                    <span className="template-row-number">{index + 1}</span>
                    <label>
                      <span>Field name</span>
                      <input
                        value={field.name}
                        disabled={isSaving}
                        maxLength={64}
                        placeholder={index === 0 ? "name" : "drug class"}
                        onChange={(event) => updateField(index, "name", event.currentTarget.value)}
                      />
                    </label>
                    <label>
                      <span>Input guidance <small>optional</small></span>
                      <input
                        value={field.description}
                        disabled={isSaving}
                        maxLength={500}
                        placeholder="What should be entered in this field?"
                        onChange={(event) =>
                          updateField(index, "description", event.currentTarget.value)
                        }
                      />
                    </label>
                    <button
                      type="button"
                      className="icon-button danger-text"
                      aria-label={`Remove field ${index + 1}`}
                      title="Remove field"
                      disabled={isSaving || draft.fields.length === 1}
                      onClick={() => {
                        setDraft((current) => ({
                          ...current,
                          fields: current.fields.filter((_, fieldIndex) => fieldIndex !== index),
                        }));
                        markDirty();
                      }}
                    >
                      ×
                    </button>
                  </article>
                ))}
              </div>
            </section>

            <section className="template-definition-card">
              <header className="template-section-heading">
                <div>
                  <span className="section-kicker">2 · Generated study cards</span>
                  <h2>Question and answer rules</h2>
                  <p>Each rule creates one ordinary Basic card whenever the template form is submitted.</p>
                </div>
                <button
                  type="button"
                  className="secondary-button"
                  disabled={isSaving || draft.rules.length >= 32}
                  onClick={() => {
                    setDraft((current) => ({
                      ...current,
                      rules: [
                        ...current.rules,
                        {
                          id: null,
                          name: `Card ${current.rules.length + 1}`,
                          frontPattern: "",
                          backPattern: "",
                        },
                      ],
                    }));
                    setActivePattern({ ruleIndex: draft.rules.length, side: "front" });
                    markDirty();
                  }}
                >
                  Add card rule
                </button>
              </header>

              <div className="placeholder-palette" aria-label="Available field placeholders">
                <span>Insert into the focused question or answer:</span>
                {placeholderNames.length === 0 ? (
                  <small>Name a field to create its placeholder.</small>
                ) : (
                  placeholderNames.map((fieldName) => (
                    <button
                      type="button"
                      key={normalizeName(fieldName)}
                      disabled={isSaving}
                      onClick={() => insertPlaceholder(fieldName)}
                    >
                      {`{{${fieldName}}}`}
                    </button>
                  ))
                )}
              </div>

              <div className="template-rule-list">
                {draft.rules.map((rule, index) => (
                  <article className="template-rule-card" key={rule.id ?? `new-rule-${index}`}>
                    <header>
                      <span className="template-row-number">{index + 1}</span>
                      <label>
                        <span>Card label</span>
                        <input
                          value={rule.name}
                          disabled={isSaving}
                          maxLength={80}
                          placeholder="Drug class"
                          onChange={(event) => updateRule(index, "name", event.currentTarget.value)}
                        />
                      </label>
                      <button
                        type="button"
                        className="icon-button danger-text"
                        aria-label={`Remove card rule ${index + 1}`}
                        disabled={isSaving || draft.rules.length === 1}
                        onClick={() => {
                          setDraft((current) => ({
                            ...current,
                            rules: current.rules.filter((_, ruleIndex) => ruleIndex !== index),
                          }));
                          setActivePattern({ ruleIndex: 0, side: "front" });
                          markDirty();
                        }}
                      >
                        ×
                      </button>
                    </header>
                    <div className="template-rule-patterns">
                      <label>
                        <span>Question pattern</span>
                        <textarea
                          value={rule.frontPattern}
                          disabled={isSaving}
                          rows={3}
                          maxLength={10000}
                          placeholder="What is the drug class of {{name}}?"
                          onFocus={() => setActivePattern({ ruleIndex: index, side: "front" })}
                          onChange={(event) =>
                            updateRule(index, "frontPattern", event.currentTarget.value)
                          }
                        />
                      </label>
                      <label>
                        <span>Answer pattern</span>
                        <textarea
                          value={rule.backPattern}
                          disabled={isSaving}
                          rows={3}
                          maxLength={10000}
                          placeholder="{{drug class}}"
                          onFocus={() => setActivePattern({ ruleIndex: index, side: "back" })}
                          onChange={(event) =>
                            updateRule(index, "backPattern", event.currentTarget.value)
                          }
                        />
                      </label>
                    </div>
                  </article>
                ))}
              </div>
            </section>
          </main>

          <aside className="template-editor-guide">
            <span className="section-kicker">Safe template language</span>
            <h2>Literal text + fields</h2>
            <p>
              A placeholder inserts the corresponding field value exactly where it appears.
              Nothing is interpreted as HTML, JavaScript, SQL, or an expression.
            </p>
            <div className="template-guide-example">
              <small>Question</small>
              <code>What are the side effects of {`{{name}}`}?</code>
              <small>Answer</small>
              <code>{`{{side effects}}`}</code>
            </div>
            <p>
              Templates are authoring shortcuts, not card owners. Submitting this form creates
              ordinary Basic cards in the chosen deck. Later edits or deletion of the template
              cannot alter cards that were already generated.
            </p>
          </aside>
        </div>
      </form>
    );
  }

  return (
    <div className="screen templates-screen">
      <header className="screen-header compact-screen-header">
        <div>
          <h1>Templates</h1>
          <p>Create reusable forms that generate several ordinary cards in one step.</p>
        </div>
        <button type="button" className="primary-button" onClick={beginCreate}>
          New template
        </button>
      </header>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
      {isLoading ? <div className="inline-progress">Loading templates…</div> : null}

      {!isLoading && templates.length === 0 ? (
        <section className="empty-library-card templates-empty-state">
          <div className="empty-icon" aria-hidden="true">{`{{ }}`}</div>
          <h2>Build a reusable entry form</h2>
          <p>
            A pharmacology template, for example, can collect one drug record and generate
            separate cards for class, side effects, and indications.
          </p>
          <button type="button" className="primary-button" onClick={beginCreate}>
            Create the first template
          </button>
        </section>
      ) : null}

      {templates.length > 0 ? (
        <section className="template-library-grid" aria-label="Card templates">
          {templates.map((template) => (
            <article className="template-library-card" key={template.id}>
              <button type="button" className="template-library-main" onClick={() => beginEdit(template)}>
                <span className="template-library-mark" aria-hidden="true">{`{{ }}`}</span>
                <span className="template-library-copy">
                  <strong>{template.name}</strong>
                  {template.description ? <span>{template.description}</span> : null}
                  <span className="template-library-stats">
                    {template.fields.length} {template.fields.length === 1 ? "field" : "fields"}
                    <span aria-hidden="true">·</span>
                    {template.rules.length} {template.rules.length === 1 ? "card rule" : "card rules"}
                  </span>
                </span>
                <span className="template-library-usage">
                  <strong>{template.rules.length}</strong>
                  <small>{template.rules.length === 1 ? "card" : "cards"} per entry</small>
                  <span>Generated cards remain independent</span>
                </span>
              </button>
              <div className="template-library-actions">
                <button type="button" onClick={() => beginEdit(template)}>Edit</button>
                <button
                  type="button"
                  className="danger-text"
                  title="Delete template; generated cards remain"
                  onClick={() => void removeTemplate(template)}
                >
                  Delete
                </button>
              </div>
            </article>
          ))}
        </section>
      ) : null}
    </div>
  );
}

async function loadTemplates(): Promise<CardTemplate[]> {
  return listCardTemplates();
}

function cloneDraft(draft: TemplateDraft): TemplateDraft {
  return {
    name: draft.name,
    description: draft.description,
    fields: draft.fields.map((field) => ({ ...field })),
    rules: draft.rules.map((rule) => ({ ...rule })),
  };
}

function collapseWhitespace(value: string): string {
  return value.trim().replace(/\s+/gu, " ");
}

function normalizeName(value: string): string {
  return collapseWhitespace(value).toLocaleLowerCase();
}

function validateDraft(draft: TemplateDraft): string | null {
  if (collapseWhitespace(draft.name) === "") return "Enter a template name.";
  if (draft.fields.length === 0) return "Add at least one field.";
  const normalizedFields = draft.fields.map((field) => normalizeName(field.name));
  if (normalizedFields.some((name) => name === "")) return "Name every field.";
  if (new Set(normalizedFields).size !== normalizedFields.length) {
    return "Each field name must be unique.";
  }
  if (draft.rules.length === 0) return "Add at least one generated-card rule.";

  const fieldNames = new Set(normalizedFields);
  for (const rule of draft.rules) {
    if (collapseWhitespace(rule.name) === "") return "Name every generated-card rule.";
    for (const [side, pattern] of [
      ["question", rule.frontPattern],
      ["answer", rule.backPattern],
    ] as const) {
      if (pattern.trim() === "") return `Enter the ${side} pattern for ${rule.name || "each rule"}.`;
      const parsed = parsePlaceholders(pattern);
      if (!parsed.valid) return "Placeholders must use the exact form {{field name}}.";
      if (parsed.names.length === 0) return `The ${side} pattern must include at least one field.`;
      const unknown = parsed.names.find((name) => !fieldNames.has(normalizeName(name)));
      if (unknown) return `The ${side} pattern references an unknown field: ${unknown}.`;
    }
  }
  return null;
}

function parsePlaceholders(value: string): { valid: boolean; names: string[] } {
  const names: string[] = [];
  const marker = /\{\{([^{}]+)\}\}/gu;
  let cursor = 0;
  for (const match of value.matchAll(marker)) {
    const index = match.index ?? 0;
    const literal = value.slice(cursor, index);
    if (literal.includes("{{") || literal.includes("}}")) return { valid: false, names };
    names.push(collapseWhitespace(match[1]));
    cursor = index + match[0].length;
  }
  const tail = value.slice(cursor);
  return {
    valid: !tail.includes("{{") && !tail.includes("}}") && names.every(Boolean),
    names,
  };
}

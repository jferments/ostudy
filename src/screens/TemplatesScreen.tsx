import { useEffect, useMemo, useRef, useState } from "react";

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

interface FieldDraft extends CardTemplateFieldInput {
  key: string;
}

interface RuleDraft extends CardTemplateRuleInput {
  key: string;
}

interface TemplateDraft {
  id?: string;
  revision?: number;
  name: string;
  description: string;
  fields: FieldDraft[];
  rules: RuleDraft[];
}

export function TemplatesScreen({
  onDirtyChange,
  showToast,
}: TemplatesScreenProps) {
  const [templates, setTemplates] = useState<CardTemplate[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [draft, setDraft] = useState<TemplateDraft | null>(null);
  const [baseline, setBaseline] = useState<TemplateDraft | null>(null);
  const savingRef = useRef(false);
  const dirty = useMemo(
    () => draft !== null && JSON.stringify(draft) !== JSON.stringify(baseline),
    [baseline, draft],
  );

  useEffect(() => {
    void loadTemplates();
  }, []);

  useEffect(() => {
    onDirtyChange(dirty);
    return () => onDirtyChange(false);
  }, [dirty, onDirtyChange]);

  async function loadTemplates(): Promise<void> {
    setIsLoading(true);
    setErrorMessage(null);
    try {
      setTemplates(await listCardTemplates());
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsLoading(false);
    }
  }

  function beginCreate(): void {
    const next = newTemplateDraft();
    setDraft(next);
    setBaseline(next);
    setErrorMessage(null);
  }

  function beginEdit(template: CardTemplate): void {
    const next = draftFromTemplate(template);
    setDraft(next);
    setBaseline(next);
    setErrorMessage(null);
  }

  function cancelEdit(): void {
    if (isSaving) return;
    if (dirty && !window.confirm("Discard the changes to this card template?")) return;
    setDraft(null);
    setBaseline(null);
    setErrorMessage(null);
  }

  async function saveTemplate(): Promise<void> {
    if (!draft || savingRef.current) return;
    const validation = validateDraft(draft);
    if (validation) {
      setErrorMessage(validation);
      return;
    }
    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      const input = {
        name: draft.name,
        description: draft.description,
        fields: draft.fields.map(({ id, name, description }) => ({
          id,
          name,
          description,
        })),
        rules: draft.rules.map(({ id, name, frontPattern, backPattern }) => ({
          id,
          name,
          frontPattern,
          backPattern,
        })),
      };
      const saved = draft.id
        ? await updateCardTemplate({
            ...input,
            templateId: draft.id,
            revision: draft.revision ?? 1,
          })
        : await createCardTemplate(input);
      setTemplates((current) => {
        const withoutSaved = current.filter((template) => template.id !== saved.id);
        return [...withoutSaved, saved].sort((left, right) =>
          left.name.localeCompare(right.name, undefined, { sensitivity: "base" }),
        );
      });
      setDraft(null);
      setBaseline(null);
      onDirtyChange(false);
      showToast("success", draft.id ? "Card template updated." : "Card template created.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      savingRef.current = false;
      setIsSaving(false);
    }
  }

  async function removeTemplate(template: CardTemplate): Promise<void> {
    if (
      !window.confirm(
        `Delete “${template.name}”?\n\nCards already generated from this template will remain in their decks.`,
      )
    ) {
      return;
    }
    setErrorMessage(null);
    try {
      await deleteCardTemplate(template.id);
      setTemplates((current) => current.filter((item) => item.id !== template.id));
      if (draft?.id === template.id) {
        setDraft(null);
        setBaseline(null);
      }
      showToast("success", `Deleted “${template.name}”. Existing cards were not changed.`);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    }
  }

  if (draft) {
    return (
      <div className="screen templates-screen" aria-busy={isSaving}>
        <header className="screen-header compact-screen-header">
          <div>
            <button type="button" className="back-button" onClick={cancelEdit}>
              ← Templates
            </button>
            <h1>{draft.id ? "Edit card template" : "New card template"}</h1>
            <p>
              Define data fields once, then use literal placeholders to generate several
              permanent Basic cards from one entry form.
            </p>
          </div>
          <div className="header-actions">
            <button
              type="button"
              className="secondary-button"
              disabled={isSaving}
              onClick={cancelEdit}
            >
              Cancel
            </button>
            <button
              type="button"
              className="primary-button"
              disabled={isSaving || !dirty}
              onClick={() => void saveTemplate()}
            >
              {isSaving ? "Saving…" : "Save template"}
            </button>
          </div>
        </header>

        {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}

        <div className="template-editor-layout">
          <section className="template-editor-section">
            <div className="section-heading compact-section-heading">
              <div>
                <h2>Template details</h2>
                <p>The template itself is an authoring shortcut, not a card type.</p>
              </div>
            </div>
            <div className="template-details-grid">
              <label>
                <span>Name</span>
                <input
                  value={draft.name}
                  maxLength={80}
                  disabled={isSaving}
                  placeholder="Pharmacology"
                  onChange={(event) =>
                    setDraft((current) =>
                      current ? { ...current, name: event.currentTarget.value } : current,
                    )
                  }
                />
              </label>
              <label>
                <span>Description <small>optional</small></span>
                <textarea
                  value={draft.description}
                  maxLength={2000}
                  rows={3}
                  disabled={isSaving}
                  placeholder="Medication facts entered once and generated as focused study cards."
                  onChange={(event) =>
                    setDraft((current) =>
                      current
                        ? { ...current, description: event.currentTarget.value }
                        : current,
                    )
                  }
                />
              </label>
            </div>
          </section>

          <section className="template-editor-section">
            <div className="section-heading compact-section-heading">
              <div>
                <h2>Input fields</h2>
                <p>Each field becomes one labeled input in the card editor.</p>
              </div>
              <button
                type="button"
                className="secondary-button"
                disabled={isSaving || draft.fields.length >= 32}
                onClick={() =>
                  setDraft((current) =>
                    current
                      ? {
                          ...current,
                          fields: [
                            ...current.fields,
                            {
                              key: createDraftKey(),
                              name: "",
                              description: "",
                            },
                          ],
                        }
                      : current,
                  )
                }
              >
                Add field
              </button>
            </div>
            <div className="template-definition-list">
              {draft.fields.map((field, index) => (
                <article key={field.key} className="template-definition-row">
                  <span className="definition-number">{index + 1}</span>
                  <label>
                    <span>Field name</span>
                    <input
                      value={field.name}
                      maxLength={64}
                      disabled={isSaving}
                      placeholder="Drug class"
                      onChange={(event) =>
                        updateField(setDraft, field.key, { name: event.currentTarget.value })
                      }
                    />
                  </label>
                  <label>
                    <span>Input guidance <small>optional</small></span>
                    <input
                      value={field.description}
                      maxLength={500}
                      disabled={isSaving}
                      placeholder="Include the therapeutic class and major subclass."
                      onChange={(event) =>
                        updateField(setDraft, field.key, {
                          description: event.currentTarget.value,
                        })
                      }
                    />
                  </label>
                  <button
                    type="button"
                    className="danger-text"
                    disabled={isSaving || draft.fields.length === 1}
                    onClick={() =>
                      setDraft((current) =>
                        current
                          ? {
                              ...current,
                              fields: current.fields.filter((item) => item.key !== field.key),
                            }
                          : current,
                      )
                    }
                  >
                    Remove
                  </button>
                </article>
              ))}
            </div>
          </section>

          <section className="template-editor-section">
            <div className="section-heading compact-section-heading">
              <div>
                <h2>Cards generated</h2>
                <p>
                  Use a complete placeholder such as <code>{"{{Name}}"}</code>. No
                  expressions, HTML, code, or commands are evaluated.
                </p>
              </div>
              <button
                type="button"
                className="secondary-button"
                disabled={isSaving || draft.rules.length >= 32}
                onClick={() =>
                  setDraft((current) =>
                    current
                      ? {
                          ...current,
                          rules: [
                            ...current.rules,
                            {
                              key: createDraftKey(),
                              name: "",
                              frontPattern: "",
                              backPattern: "",
                            },
                          ],
                        }
                      : current,
                  )
                }
              >
                Add card rule
              </button>
            </div>

            <div className="template-placeholder-strip" aria-label="Available placeholders">
              <span>Available:</span>
              {draft.fields
                .filter((field) => field.name.trim() !== "")
                .map((field) => (
                  <code key={field.key}>{`{{${field.name.trim()}}}`}</code>
                ))}
            </div>

            <div className="template-rule-list">
              {draft.rules.map((rule, index) => (
                <article key={rule.key} className="template-rule-card">
                  <header>
                    <span className="definition-number">{index + 1}</span>
                    <label>
                      <span>Rule name</span>
                      <input
                        value={rule.name}
                        maxLength={80}
                        disabled={isSaving}
                        placeholder="Drug class"
                        onChange={(event) =>
                          updateRule(setDraft, rule.key, { name: event.currentTarget.value })
                        }
                      />
                    </label>
                    <button
                      type="button"
                      className="danger-text"
                      disabled={isSaving || draft.rules.length === 1}
                      onClick={() =>
                        setDraft((current) =>
                          current
                            ? {
                                ...current,
                                rules: current.rules.filter((item) => item.key !== rule.key),
                              }
                            : current,
                        )
                      }
                    >
                      Remove
                    </button>
                  </header>
                  <div className="template-rule-sides">
                    <label>
                      <span>Question pattern</span>
                      <textarea
                        value={rule.frontPattern}
                        rows={3}
                        maxLength={4000}
                        disabled={isSaving}
                        placeholder="What is the drug class of {{Name}}?"
                        onChange={(event) =>
                          updateRule(setDraft, rule.key, {
                            frontPattern: event.currentTarget.value,
                          })
                        }
                      />
                    </label>
                    <label>
                      <span>Answer pattern</span>
                      <textarea
                        value={rule.backPattern}
                        rows={3}
                        maxLength={4000}
                        disabled={isSaving}
                        placeholder="{{Drug class}}"
                        onChange={(event) =>
                          updateRule(setDraft, rule.key, {
                            backPattern: event.currentTarget.value,
                          })
                        }
                      />
                    </label>
                  </div>
                </article>
              ))}
            </div>
          </section>
        </div>
      </div>
    );
  }

  return (
    <div className="screen templates-screen">
      <header className="screen-header compact-screen-header">
        <div>
          <h1>Templates</h1>
          <p>
            Build reusable forms that turn one set of facts into multiple permanent
            Basic flashcards.
          </p>
        </div>
        <button type="button" className="primary-button" onClick={beginCreate}>
          New template
        </button>
      </header>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
      {isLoading ? <div className="inline-progress">Loading templates…</div> : null}

      {!isLoading && templates.length === 0 ? (
        <section className="compact-empty-state large-empty-state">
          <div className="empty-icon" aria-hidden="true">▧</div>
          <h2>Create a reusable card template</h2>
          <p>
            A template can collect fields such as medication name, drug class, side
            effects, and conditions, then generate several focused cards at once.
          </p>
          <button type="button" className="primary-button" onClick={beginCreate}>
            Create template
          </button>
        </section>
      ) : null}

      {templates.length > 0 ? (
        <section className="template-library-grid" aria-label="Card templates">
          {templates.map((template) => (
            <article className="template-library-card" key={template.id}>
              <button
                type="button"
                className="template-library-main"
                onClick={() => beginEdit(template)}
              >
                <span className="template-library-mark" aria-hidden="true">▧</span>
                <span>
                  <strong>{template.name}</strong>
                  {template.description ? <small>{template.description}</small> : null}
                </span>
                <span className="template-library-counts">
                  <b>{template.fields.length}</b> fields
                  <b>{template.rules.length}</b> cards
                </span>
              </button>
              <div className="template-library-actions">
                <button type="button" onClick={() => beginEdit(template)}>
                  Edit
                </button>
                <button
                  type="button"
                  className="danger-text"
                  onClick={() => void removeTemplate(template)}
                >
                  Delete
                </button>
              </div>
            </article>
          ))}
        </section>
      ) : null}

      <section className="template-safety-note">
        <strong>Templates are safe authoring tools, not card containers.</strong>
        <p>
          Placeholders substitute plain field values only. Generated cards are stored in
          their destination deck and remain there even when the template is edited or
          deleted.
        </p>
      </section>
    </div>
  );
}

function newTemplateDraft(): TemplateDraft {
  return {
    name: "",
    description: "",
    fields: [
      {
        key: createDraftKey(),
        name: "Name",
        description: "The main subject used by the generated questions.",
      },
    ],
    rules: [
      {
        key: createDraftKey(),
        name: "Definition",
        frontPattern: "What is {{Name}}?",
        backPattern: "{{Name}}",
      },
    ],
  };
}

function draftFromTemplate(template: CardTemplate): TemplateDraft {
  return {
    id: template.id,
    revision: template.revision,
    name: template.name,
    description: template.description,
    fields: template.fields.map((field) => ({
      id: field.id,
      key: field.id,
      name: field.name,
      description: field.description,
    })),
    rules: template.rules.map((rule) => ({
      id: rule.id,
      key: rule.id,
      name: rule.name,
      frontPattern: rule.frontPattern,
      backPattern: rule.backPattern,
    })),
  };
}

function validateDraft(draft: TemplateDraft): string | null {
  if (draft.name.trim() === "") return "Enter a template name.";
  if (draft.fields.some((field) => field.name.trim() === "")) {
    return "Give every input field a name.";
  }
  const normalizedFields = draft.fields.map((field) =>
    field.name.trim().replace(/\s+/gu, " ").toLocaleLowerCase(),
  );
  if (new Set(normalizedFields).size !== normalizedFields.length) {
    return "Input field names must be unique.";
  }
  if (
    draft.rules.some(
      (rule) =>
        rule.name.trim() === "" ||
        rule.frontPattern.trim() === "" ||
        rule.backPattern.trim() === "",
    )
  ) {
    return "Give every card rule a name, question pattern, and answer pattern.";
  }
  return null;
}

function updateField(
  setDraft: React.Dispatch<React.SetStateAction<TemplateDraft | null>>,
  key: string,
  patch: Partial<FieldDraft>,
): void {
  setDraft((current) =>
    current
      ? {
          ...current,
          fields: current.fields.map((field) =>
            field.key === key ? { ...field, ...patch } : field,
          ),
        }
      : current,
  );
}

function updateRule(
  setDraft: React.Dispatch<React.SetStateAction<TemplateDraft | null>>,
  key: string,
  patch: Partial<RuleDraft>,
): void {
  setDraft((current) =>
    current
      ? {
          ...current,
          rules: current.rules.map((rule) =>
            rule.key === key ? { ...rule, ...patch } : rule,
          ),
        }
      : current,
  );
}

function createDraftKey(): string {
  return crypto.randomUUID();
}

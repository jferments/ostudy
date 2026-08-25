import { useEffect, useMemo, useRef, useState } from "react";

import type { ShowToast } from "../components/Toast";
import { TemplatesController } from "../controllers/templatesController";
import { useDurableDraft } from "../controllers/useDurableDraft";
import { collectAssetIds, EMPTY_RICH_DOCUMENT, cloneDocument } from "../editor/richText";
import {
  type Asset,
} from "../ipc/assets";
import {
  getErrorMessage,
  type RichDocument,
} from "../ipc/common";
import {
  type CardTemplate,
  type CardTemplateFieldInput,
  type CardTemplateRuleInput,
  type CardTemplateSummary,
} from "../ipc/templates";
import { useConfirm } from "../ui/overlays/overlayContext";
import { TemplateLayoutEditor } from "./templates/TemplateLayoutEditor";
import { Button } from "../ui/primitives/Button";
import { EmptyState } from "../ui/primitives/EmptyState";
import { InlineNotice, InlineProgress } from "../ui/primitives/InlineNotice";

interface TemplatesScreenProps {
  onBack?: () => void | Promise<void>;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

interface FieldDraft extends CardTemplateFieldInput {
  id: string;
  key: string;
  position: number;
}

interface RuleDraft extends CardTemplateRuleInput {
  id: string;
  key: string;
  position: number;
}

interface TemplateDraft {
  id?: string;
  revision?: number;
  name: string;
  description: string;
  fields: FieldDraft[];
  rules: RuleDraft[];
  assets: Asset[];
}

export function TemplatesScreen({ onBack, onDirtyChange, showToast }: TemplatesScreenProps) {
  const [templates, setTemplates] = useState<CardTemplateSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isOpening, setIsOpening] = useState(false);
  const [isOpeningAssetDraft, setIsOpeningAssetDraft] = useState(false);
  const openRequestSequence = useRef(0);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [draft, setDraft] = useState<TemplateDraft | null>(null);
  const [baseline, setBaseline] = useState<TemplateDraft | null>(null);
  const [assetDraftId, setAssetDraftId] = useState<string | null>(null);
  const assetDraftRef = useRef<string | null>(null);
  const savingRef = useRef(false);
  const controllerRef = useRef<TemplatesController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new TemplatesController();
  const controller = controllerRef.current;
  const confirm = useConfirm();
  const dirty = useMemo(
    () => draft !== null && JSON.stringify(draft) !== JSON.stringify(baseline),
    [baseline, draft],
  );
  const usedAssetIds = useMemo(() => {
    const ids = new Set<string>();
    for (const rule of draft?.rules ?? []) {
      for (const id of collectAssetIds(rule.frontDoc)) ids.add(id);
      for (const id of collectAssetIds(rule.backDoc)) ids.add(id);
    }
    return [...ids].sort();
  }, [draft?.rules]);
  const usedAssetKey = usedAssetIds.join(",");
  const durableDraft = useDurableDraft<TemplateDraft & Record<string, unknown>>({
    kind: "template",
    targetEntityId: draft?.id,
    contextEntityId: draft?.id ? undefined : "0".repeat(32),
    baseRevision: draft?.revision,
    payload: (draft ?? newTemplateDraft()) as TemplateDraft & Record<string, unknown>,
    assetIds: usedAssetIds,
    enabled: draft !== null,
    onRecovered: (payload) => setDraft(payload),
  });

  useEffect(() => {
    void loadTemplates();
    return () => controller.invalidate();
  }, [controller]);

  useEffect(() => {
    onDirtyChange(durableDraft.hasUnsavedLocalChanges);
    return () => onDirtyChange(false);
  }, [durableDraft.hasUnsavedLocalChanges, onDirtyChange]);

  useEffect(() => {
    assetDraftRef.current = durableDraft.assetDraftId;
    setAssetDraftId(durableDraft.assetDraftId);
    setIsOpeningAssetDraft(durableDraft.status === "opening");
  }, [durableDraft.assetDraftId, durableDraft.status]);



  useEffect(() => {
    if (!assetDraftId) return;
    const retained = usedAssetKey === "" ? [] : usedAssetKey.split(",");
    const timer = window.setTimeout(() => {
      void controller.syncDraft(assetDraftId, retained).catch((error: unknown) => {
        if (assetDraftRef.current === assetDraftId) {
          setErrorMessage(`The image draft could not be synchronized: ${getErrorMessage(error)}`);
        }
      });
    }, 250);
    return () => window.clearTimeout(timer);
  }, [assetDraftId, usedAssetKey]);

  async function loadTemplates(): Promise<void> {
    setIsLoading(true);
    setErrorMessage(null);
    try {
      const loaded = await controller.list();
      if (loaded) setTemplates(loaded);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsLoading(false);
    }
  }

  async function openDraft(next: TemplateDraft): Promise<void> {
    setDraft(next);
    setBaseline(next);
    setErrorMessage(null);
  }

  function beginCreate(): void {
    openRequestSequence.current += 1;
    void openDraft(newTemplateDraft());
  }

  async function beginEdit(summary: CardTemplateSummary): Promise<void> {
    if (isOpening) return;
    const requestId = openRequestSequence.current + 1;
    openRequestSequence.current = requestId;
    setIsOpening(true);
    setErrorMessage(null);
    try {
      const template = await controller.template(summary.id);
      if (!template) return;
      if (openRequestSequence.current !== requestId) return;
      await openDraft(draftFromTemplate(template));
    } catch (error: unknown) {
      if (openRequestSequence.current === requestId) setErrorMessage(getErrorMessage(error));
    } finally {
      if (openRequestSequence.current === requestId) setIsOpening(false);
    }
  }

  async function cancelEdit(): Promise<void> {
    if (isSaving) return;
    if (dirty) {
      const accepted = await confirm({
        title: "Discard template changes?",
        description: "The unsaved fields, card rules, and layout changes will be lost.",
        confirmLabel: "Discard changes",
        destructive: true,
      });
      if (!accepted) return;
    }
    openRequestSequence.current += 1;
    setIsOpeningAssetDraft(false);
    try {
      await durableDraft.discard();
    } catch (error: unknown) {
      setErrorMessage(`The durable template draft could not be discarded: ${getErrorMessage(error)}`);
      return;
    }
    assetDraftRef.current = null;
    setAssetDraftId(null);
    setDraft(null);
    setBaseline(null);
    setErrorMessage(null);
  }

  async function saveTemplate(): Promise<void> {
    if (!draft || savingRef.current) return;
    if (!assetDraftId) {
      setErrorMessage("The managed image draft is not ready yet. Try saving again after it opens.");
      return;
    }
    const validation = validateDraft(draft);
    if (validation) {
      setErrorMessage(validation);
      return;
    }
    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      const editorDraft = await durableDraft.prepareCanonicalCommit();
      const input = {
        name: draft.name,
        description: draft.description,
        fields: draft.fields.map(({ id, name, description }) => ({ id, name, description })),
        rules: draft.rules.map(({ id, name, frontDoc, backDoc }) => ({
          id,
          name,
          frontDoc,
          backDoc,
        })),
        assetDraftId,
        editorDraft,
      };
      if (draft.id) {
        await controller.update({
          ...input,
          templateId: draft.id,
          revision: draft.revision ?? 1,
        });
      } else {
        await controller.create(input);
      }
      durableDraft.acceptCanonicalCommit();
      openRequestSequence.current += 1;
      assetDraftRef.current = null;
      setAssetDraftId(null);
      setDraft(null);
      setBaseline(null);
      onDirtyChange(false);
      await loadTemplates();
      showToast("success", draft.id ? "Card template updated." : "Card template created.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      savingRef.current = false;
      setIsSaving(false);
    }
  }

  async function removeTemplate(template: CardTemplateSummary): Promise<void> {
    const accepted = await confirm({
      title: `Delete “${template.name}”?`,
      description: "Cards already generated from this template will remain in their decks.",
      confirmLabel: "Delete template",
      destructive: true,
    });
    if (!accepted) return;
    setErrorMessage(null);
    try {
      await controller.delete(template.id);
      setTemplates((current) => current.filter((item) => item.id !== template.id));
      if (draft?.id === template.id) {
        const imageDraft = assetDraftRef.current;
        assetDraftRef.current = null;
        setAssetDraftId(null);
        if (imageDraft) await controller.cancelDraft(imageDraft);
        setDraft(null);
        setBaseline(null);
      }
      showToast("success", `Deleted “${template.name}”. Existing cards were not changed.`);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    }
  }

  function addAsset(asset: Asset): void {
    setDraft((current) =>
      current && !current.assets.some((item) => item.id === asset.id)
        ? { ...current, assets: [...current.assets, asset] }
        : current,
    );
  }

  if (draft) {
    return (
      <div className="screen templates-screen" aria-busy={isSaving || isOpeningAssetDraft}>
        <header className="screen-header compact-screen-header">
          <div>
            <Button type="button" variant="back" onClick={() => void cancelEdit()}>
              ← Templates
            </Button>
            <h1>{draft.id ? "Edit card template" : "New card template"}</h1>
            <p>
              Define reusable rich fields and design the question and answer sides of every card
              this form generates.
            </p>
          </div>
          <div className="header-actions">
            <Button
              type="button"
              variant="secondary"
              disabled={isSaving}
              onClick={() => void cancelEdit()}
            >
              Cancel
            </Button>
            <Button
              type="button"
              variant="primary"
              disabled={isSaving || isOpeningAssetDraft || !assetDraftId || !dirty}
              onClick={() => void saveTemplate()}
            >
              {isSaving ? "Saving…" : isOpeningAssetDraft ? "Preparing…" : "Save template"}
            </Button>
          </div>
        </header>

        {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}

        <div className="template-editor-layout">
          <section className="template-editor-section">
            <div className="section-heading compact-section-heading">
              <div>
                <h2>Template details</h2>
                <p>Name this reusable entry form and optionally describe its purpose.</p>
              </div>
            </div>
            <div className="template-details-grid">
              <label>
                <span>Name</span>
                <input
                  value={draft.name}
                  maxLength={80}
                  disabled={isSaving}
                  placeholder="Template name"
                  onChange={(event) => {
                    const value = event.currentTarget.value;
                    setDraft((current) => (current ? { ...current, name: value } : current));
                  }}
                />
              </label>
              <label>
                <span>
                  Description <small>optional</small>
                </span>
                <textarea
                  value={draft.description}
                  maxLength={2000}
                  rows={3}
                  disabled={isSaving}
                  placeholder="Describe what this template helps you enter."
                  onChange={(event) => {
                    const value = event.currentTarget.value;
                    setDraft((current) =>
                      current ? { ...current, description: value } : current,
                    );
                  }}
                />
              </label>
            </div>
          </section>

          <section className="template-editor-section">
            <div className="section-heading compact-section-heading">
              <div>
                <h2>Fields</h2>
                <p>Each field becomes a full rich-text editor when this template is used.</p>
              </div>
              <Button
                type="button"
                variant="secondary"
                disabled={isSaving || draft.fields.length >= 32}
                onClick={() =>
                  setDraft((current) =>
                    current
                      ? {
                          ...current,
                          fields: [...current.fields, createFieldDraft(current.fields.length)],
                        }
                      : current,
                  )
                }
              >
                Add field
              </Button>
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
                      placeholder="Field name"
                      onChange={(event) =>
                        updateField(setDraft, field.key, { name: event.currentTarget.value })
                      }
                    />
                  </label>
                  <label>
                    <span>
                      Field description <small>optional</small>
                    </span>
                    <input
                      value={field.description}
                      maxLength={500}
                      disabled={isSaving}
                      placeholder="Enter a description of this field."
                      onChange={(event) =>
                        updateField(setDraft, field.key, {
                          description: event.currentTarget.value,
                        })
                      }
                    />
                  </label>
                  <Button
                    type="button"
                    variant="dangerGhost"
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
                  </Button>
                </article>
              ))}
            </div>
          </section>

          <section className="template-editor-section">
            <div className="section-heading compact-section-heading">
              <div>
                <h2>Cards generated</h2>
                <p>
                  Type field names in double braces, such as {"{{Subject}}"}. The renderer keeps
                  simple values inline and automatically promotes structured values such as lists,
                  images, headings, block quotes, code blocks, and multiple paragraphs to blocks.
                </p>
              </div>
              <Button
                type="button"
                variant="secondary"
                disabled={isSaving || draft.rules.length >= 32}
                onClick={() =>
                  setDraft((current) =>
                    current
                      ? {
                          ...current,
                          rules: [...current.rules, createRuleDraft(current.rules.length)],
                        }
                      : current,
                  )
                }
              >
                Add card rule
              </Button>
            </div>

            <div className="template-rule-list rich-template-rule-list">
              {draft.rules.map((rule, index) => (
                <article key={rule.key} className="template-rule-card rich-template-rule-card">
                  <header>
                    <span className="definition-number">{index + 1}</span>
                    <label>
                      <span>Card name</span>
                      <input
                        value={rule.name}
                        maxLength={80}
                        disabled={isSaving}
                        placeholder="Card name"
                        onChange={(event) =>
                          updateRule(setDraft, rule.key, { name: event.currentTarget.value })
                        }
                      />
                    </label>
                    <Button
                      type="button"
                      variant="dangerGhost"
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
                    </Button>
                  </header>
                  <TemplateLayoutEditor
                    label="Question layout"
                    document={rule.frontDoc}
                    assets={draft.assets}
                    assetDraftId={assetDraftId}
                    disabled={isSaving || isOpeningAssetDraft}
                    onAssetImported={addAsset}
                    onChange={(frontDoc) => updateRule(setDraft, rule.key, { frontDoc })}
                  />
                  <TemplateLayoutEditor
                    label="Answer layout"
                    document={rule.backDoc}
                    assets={draft.assets}
                    assetDraftId={assetDraftId}
                    disabled={isSaving || isOpeningAssetDraft}
                    onAssetImported={addAsset}
                    onChange={(backDoc) => updateRule(setDraft, rule.key, { backDoc })}
                  />
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
          {onBack ? (
            <Button type="button" variant="back" onClick={() => void onBack()}>
              ← Decks
            </Button>
          ) : null}
          <h1>Templates</h1>
          <p>Build reusable rich entry forms that generate permanent Basic cards.</p>
        </div>
        <Button type="button" variant="primary" onClick={beginCreate}>
          New template
        </Button>
      </header>

      {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
      {isLoading ? <InlineProgress>Loading templates…</InlineProgress> : null}
      {isOpening ? <InlineProgress>Opening template…</InlineProgress> : null}

      {!isLoading && templates.length === 0 ? (
        <EmptyState
          title="Create a reusable card template"
          description="Define the facts you enter once, then design one or more cards that use those fields as rich text, lists, or images."
          icon="▧"
          actions={
            <Button type="button" variant="primary" onClick={beginCreate}>
              Create template
            </Button>
          }
        />
      ) : null}

      {templates.length > 0 ? (
        <section className="template-library-grid" aria-label="Card templates">
          {templates.map((template) => (
            <article className="template-library-card" key={template.id}>
              <button
                type="button"
                className="template-library-main"
                disabled={isOpening}
                onClick={() => void beginEdit(template)}
              >
                <span className="template-library-mark" aria-hidden="true">
                  ▧
                </span>
                <span>
                  <strong>{template.name}</strong>
                  {template.description ? <small>{template.description}</small> : null}
                </span>
                <span className="template-library-counts">
                  <b>{template.fieldCount}</b> fields
                  <b>{template.generatedCardRuleCount}</b> cards
                  {template.imageCount > 0 ? <small>{template.imageCount} images</small> : null}
                </span>
              </button>
              <div className="template-library-actions">
                <button type="button" disabled={isOpening} onClick={() => void beginEdit(template)}>
                  Edit
                </button>
                <Button
                  type="button"
                  variant="dangerGhost"
                  onClick={() => void removeTemplate(template)}
                >
                  Delete
                </Button>
              </div>
            </article>
          ))}
        </section>
      ) : null}
    </div>
  );
}

function newTemplateDraft(): TemplateDraft {
  const field = createFieldDraft(0);
  return {
    name: "",
    description: "",
    fields: [field],
    rules: [createRuleDraft(0, field)],
    assets: [],
  };
}

function createFieldDraft(position: number): FieldDraft {
  const id = createEntityId();
  return {
    id,
    key: id,
    name: position === 0 ? "Field" : "",
    description: "",
    position,
  };
}

function createRuleDraft(position: number, field?: FieldDraft): RuleDraft {
  const id = createEntityId();
  const fieldId = field?.id;
  const label = field?.name || "Field";
  return {
    id,
    key: id,
    name: position === 0 ? "Card" : "",
    frontDoc: fieldId
      ? documentWithInlineField(fieldId, label, "Question about ")
      : cloneDocument(EMPTY_RICH_DOCUMENT),
    backDoc: fieldId
      ? documentWithInlineField(fieldId, label)
      : cloneDocument(EMPTY_RICH_DOCUMENT),
    position,
  };
}

function documentWithInlineField(
  fieldId: string,
  label: string,
  prefix = "",
): RichDocument {
  const content: Array<Record<string, unknown>> = [];
  if (prefix !== "") content.push({ type: "text", text: prefix });
  content.push({ type: "templateFieldInline", attrs: { fieldId, label } });
  return { type: "doc", content: [{ type: "paragraph", content }] };
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
      position: field.position,
    })),
    rules: template.rules.map((rule) => ({
      id: rule.id,
      key: rule.id,
      name: rule.name,
      frontDoc: cloneDocument(rule.frontDoc),
      backDoc: cloneDocument(rule.backDoc),
      position: rule.position,
    })),
    assets: template.assets,
  };
}

function validateDraft(draft: TemplateDraft): string | null {
  if (draft.name.trim() === "") return "Enter a template name.";
  if (draft.fields.some((field) => field.name.trim() === "")) return "Give every field a name.";
  const normalizedFields = draft.fields.map((field) =>
    field.name.trim().replace(/\s+/gu, " ").toLocaleLowerCase(),
  );
  if (new Set(normalizedFields).size !== normalizedFields.length) {
    return "Field names must be unique.";
  }
  if (draft.rules.some((rule) => rule.name.trim() === "")) {
    return "Give every generated card a name.";
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

function createEntityId(): string {
  return crypto.randomUUID().replaceAll("-", "");
}

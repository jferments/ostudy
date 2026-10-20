import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { ShowToast } from "../components/Toast";
import { TemplatesController } from "../controllers/templatesController";
import { useDurableDraft } from "../controllers/useDurableDraft";
import { collectAssetIds, EMPTY_RICH_DOCUMENT, cloneDocument } from "../editor/richText";
import type { Asset } from "../ipc/assets";
import { getErrorMessage, type RichDocument } from "../ipc/common";
import type { CardTemplate, CardTemplateFieldInput, CardTemplateRuleInput, CardTemplateSummary } from "../ipc/templates";
import { useConfirm } from "../ui/overlays/overlayContext";
import { OverflowMenu } from "../ui/overlays/OverflowMenu";
import { Icon } from "../ui/icons/Icon";
import { ListRow, ListRowMedia, ListRows } from "../ui/primitives/ListRow";
import { PageScaffold, PageHeader } from "../ui/patterns/WorkspacePatterns";
import { Button } from "../ui/primitives/Button";
import { Badge } from "../ui/primitives/Badge";
import { Panel } from "../ui/primitives/Panel";
import { FormField } from "../ui/primitives/FormField";
import { TextField } from "../ui/primitives/TextField";
import { Textarea } from "../ui/primitives/FormControls";
import { InlineNotice, InlineProgress } from "../ui/primitives/InlineNotice";
import { TemplateLayoutEditor } from "./templates/TemplateLayoutEditor";

interface TemplatesScreenProps {
  onBack?: () => void | Promise<void>;
  /** The built-in formats, listed above the learner's own templates. */
  builtIn?: ReactNode;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}
interface FieldDraft extends CardTemplateFieldInput { id: string; key: string; position: number; }
interface RuleDraft extends CardTemplateRuleInput { id: string; key: string; position: number; }
interface TemplateDraft {
  id?: string; revision?: number; name: string; description: string;
  fields: FieldDraft[]; rules: RuleDraft[]; assets: Asset[];
}

export function TemplatesScreen({ onBack, builtIn, onDirtyChange, showToast }: TemplatesScreenProps) {
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
  const dirty = useMemo(() => draft !== null && JSON.stringify(draft) !== JSON.stringify(baseline), [baseline, draft]);
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
    kind: "template", targetEntityId: draft?.id,
    contextEntityId: draft?.id ? undefined : "0".repeat(32), baseRevision: draft?.revision,
    payload: (draft ?? newTemplateDraft()) as TemplateDraft & Record<string, unknown>,
    assetIds: usedAssetIds, enabled: draft !== null, onRecovered: (payload) => setDraft(payload),
  });
  const loadTemplates = useCallback(async (): Promise<void> => {
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
  }, [controller]);

  useEffect(() => {
    void loadTemplates();
    return () => controller.invalidate();
  }, [controller, loadTemplates]);
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
        if (assetDraftRef.current === assetDraftId) setErrorMessage(`The image draft could not be synchronized: ${getErrorMessage(error)}`);
      });
    }, 250);
    return () => window.clearTimeout(timer);
  }, [assetDraftId, controller, usedAssetKey]);

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
      if (!template || openRequestSequence.current !== requestId) return;
      await openDraft(draftFromTemplate(template));
    } catch (error: unknown) {
      if (openRequestSequence.current === requestId) setErrorMessage(getErrorMessage(error));
    } finally { if (openRequestSequence.current === requestId) setIsOpening(false); }
  }
  async function cancelEdit(): Promise<void> {
    if (isSaving) return;
    if (dirty) {
      const accepted = await confirm({
        title: "Discard template changes?", description: "The unsaved fields, card rules, and layout changes will be lost.",
        confirmLabel: "Discard changes", destructive: true,
      });
      if (!accepted) return;
    }
    openRequestSequence.current += 1;
    setIsOpeningAssetDraft(false);
    try { await durableDraft.discard(); }
    catch (error: unknown) {
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
    if (validation) { setErrorMessage(validation); return; }
    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      const editorDraft = await durableDraft.prepareCanonicalCommit();
      const input = {
        name: draft.name, description: draft.description,
        fields: draft.fields.map(({ id, name, description }) => ({ id, name, description })),
        rules: draft.rules.map(({ id, name, frontDoc, backDoc }) => ({ id, name, frontDoc, backDoc })),
        assetDraftId, editorDraft,
      };
      if (draft.id) await controller.update({ ...input, templateId: draft.id, revision: draft.revision ?? 1 });
      else await controller.create(input);
      durableDraft.acceptCanonicalCommit();
      openRequestSequence.current += 1;
      assetDraftRef.current = null;
      setAssetDraftId(null);
      setDraft(null);
      setBaseline(null);
      onDirtyChange(false);
      await loadTemplates();
      showToast("success", draft.id ? "Card template updated." : "Card template created.");
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
    finally { savingRef.current = false; setIsSaving(false); }
  }
  async function removeTemplate(template: CardTemplateSummary): Promise<void> {
    const accepted = await confirm({
      title: `Delete “${template.name}”?`, description: "Cards already generated from this template will remain in their decks.",
      confirmLabel: "Delete template", destructive: true,
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
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
  }
  function addAsset(asset: Asset): void {
    setDraft((current) => current && !current.assets.some((item) => item.id === asset.id)
      ? { ...current, assets: [...current.assets, asset] } : current);
  }

  if (draft) {
    return <PageScaffold width="wide" aria-busy={isSaving || isOpeningAssetDraft}>
      <Button variant="back" disabled={isSaving} onClick={() => void cancelEdit()}>← Templates</Button>
      <PageHeader title={draft.id ? "Edit card template" : "New card template"} eyebrow="Reusable authoring" description="Define reusable rich fields and design the question and answer sides of every card this form generates."
        actions={<><Button disabled={isSaving} onClick={() => void cancelEdit()}>Cancel</Button><Button variant="primary" disabled={isSaving || isOpeningAssetDraft || !assetDraftId || !dirty} onClick={() => void saveTemplate()}>{isSaving ? "Saving…" : isOpeningAssetDraft ? "Preparing…" : "Save template"}</Button></>} />
      {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
      <div className="tw:grid tw:gap-6">
        <Panel title="Template details" description="Name this reusable entry form and optionally describe its purpose.">
          <div className="tw:grid tw:gap-4 tw:lg:grid-cols-2">
            <FormField label="Name"><TextField value={draft.name} maxLength={80} disabled={isSaving} placeholder="Template name" onChange={(event) => {
              const value = event.currentTarget.value; setDraft((current) => current ? { ...current, name: value } : current);
            }} /></FormField>
            <FormField label="Description" description="Optional"><Textarea value={draft.description} maxLength={2000} rows={3} disabled={isSaving} placeholder="Describe what this template helps you enter." onChange={(event) => {
              const value = event.currentTarget.value; setDraft((current) => current ? { ...current, description: value } : current);
            }} /></FormField>
          </div>
        </Panel>
        <Panel title="Fields" description="Each field becomes a full rich-text editor when this template is used."
          actions={<Button disabled={isSaving || draft.fields.length >= 32} onClick={() => setDraft((current) => current ? { ...current, fields: [...current.fields, createFieldDraft(current.fields.length)] } : current)}>Add field</Button>}>
          {draft.fields.map((field, index) => <article key={field.key} className="tw:grid tw:items-end tw:gap-4 tw:rounded-lg tw:bg-muted tw:p-4 tw:lg:grid-cols-[auto_1fr_2fr_auto]">
            <Badge>{index + 1}</Badge>
            <FormField label="Field name"><TextField value={field.name} maxLength={64} disabled={isSaving} placeholder="Field name" onChange={(event) => updateField(setDraft, field.key, { name: event.currentTarget.value })} /></FormField>
            <FormField label="Field description" description="Optional"><TextField value={field.description} maxLength={500} disabled={isSaving} placeholder="Enter a description of this field." onChange={(event) => updateField(setDraft, field.key, { description: event.currentTarget.value })} /></FormField>
            <Button variant="dangerGhost" disabled={isSaving || draft.fields.length === 1} onClick={() => setDraft((current) => current ? { ...current, fields: current.fields.filter((item) => item.key !== field.key) } : current)}>Remove</Button>
          </article>)}
        </Panel>
        <Panel title="Cards generated" description={'Type field names in double braces, such as {{Subject}}. Simple values stay inline; lists, images, headings, block quotes, code blocks and multiple paragraphs become blocks automatically.'}
          actions={<Button disabled={isSaving || draft.rules.length >= 32} onClick={() => setDraft((current) => current ? { ...current, rules: [...current.rules, createRuleDraft(current.rules.length)] } : current)}>Add card rule</Button>}>
          {draft.rules.map((rule, index) => <article key={rule.key} className="tw:grid tw:min-w-0 tw:gap-5 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-4">
            <header className="tw:grid tw:items-end tw:gap-3 tw:sm:grid-cols-[auto_1fr_auto]"><Badge>{index + 1}</Badge>
              <FormField label="Card name"><TextField value={rule.name} maxLength={80} disabled={isSaving} placeholder="Card name" onChange={(event) => updateRule(setDraft, rule.key, { name: event.currentTarget.value })} /></FormField>
              <Button variant="dangerGhost" disabled={isSaving || draft.rules.length === 1} onClick={() => setDraft((current) => current ? { ...current, rules: current.rules.filter((item) => item.key !== rule.key) } : current)}>Remove</Button>
            </header>
            <TemplateLayoutEditor label="Question layout" document={rule.frontDoc} assets={draft.assets} assetDraftId={assetDraftId} disabled={isSaving || isOpeningAssetDraft} onAssetImported={addAsset} onChange={(frontDoc) => updateRule(setDraft, rule.key, { frontDoc })} />
            <TemplateLayoutEditor label="Answer layout" document={rule.backDoc} assets={draft.assets} assetDraftId={assetDraftId} disabled={isSaving || isOpeningAssetDraft} onAssetImported={addAsset} onChange={(backDoc) => updateRule(setDraft, rule.key, { backDoc })} />
          </article>)}
        </Panel>
      </div>
    </PageScaffold>;
  }

  return <div className="tw:grid tw:min-w-0 tw:gap-5">
    {onBack ? <Button variant="back" className="tw:justify-self-start" onClick={() => void onBack()}>← Decks</Button> : null}
    {builtIn}
    <Panel title="Your templates" aria-busy={isLoading || isOpening}
      actions={<Button variant="secondary" onClick={beginCreate}><Icon name="plus" size={16} />New template</Button>}>
      {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
      {isLoading ? <InlineProgress>Loading templates…</InlineProgress> : null}
      {isOpening ? <InlineProgress>Opening template…</InlineProgress> : null}
      {templates.length > 0 ? <ListRows aria-label="Your templates">
        {templates.map((template) => <ListRow key={template.id} className="tw:items-center"
          leading={<ListRowMedia label={initials(template.name)} className="tw:normal-case" />}
          title={template.name}
          meta={[`${template.fieldCount} ${template.fieldCount === 1 ? "field" : "fields"}`, `${template.generatedCardRuleCount} ${template.generatedCardRuleCount === 1 ? "card" : "cards"}`,
            ...(template.imageCount > 0 ? [`${template.imageCount} ${template.imageCount === 1 ? "image" : "images"}`] : [])].join(" · ")}
          onActivate={() => void beginEdit(template)} activateDisabled={isOpening}
          trailing={<OverflowMenu label={`More for ${template.name}`} items={[
            { id: "edit", label: "Edit", disabled: isOpening, onSelect: () => void beginEdit(template) },
            { id: "delete", label: "Delete", destructive: true, onSelect: () => void removeTemplate(template) },
          ]} />} />)}
      </ListRows> : null}
    </Panel>
  </div>;
}

/** The first letters of a template's first two words, for its row tile. */
function initials(name: string): string {
  const words = name.trim().split(/\s+/u).filter(Boolean);
  const letters = words.length > 1 ? `${words[0]?.[0] ?? ""}${words[1]?.[0] ?? ""}` : (words[0] ?? "").slice(0, 2);
  return letters.charAt(0).toUpperCase() + letters.slice(1).toLowerCase();
}

function newTemplateDraft(): TemplateDraft {
  const field = createFieldDraft(0);
  return { name: "", description: "", fields: [field], rules: [createRuleDraft(0, field)], assets: [] };
}
function createFieldDraft(position: number): FieldDraft {
  const id = createEntityId();
  return { id, key: id, name: position === 0 ? "Field" : "", description: "", position };
}
function createRuleDraft(position: number, field?: FieldDraft): RuleDraft {
  const id = createEntityId();
  const fieldId = field?.id;
  const label = field?.name || "Field";
  return {
    id, key: id, name: position === 0 ? "Card" : "",
    frontDoc: fieldId ? documentWithInlineField(fieldId, label, "Question about ") : cloneDocument(EMPTY_RICH_DOCUMENT),
    backDoc: fieldId ? documentWithInlineField(fieldId, label) : cloneDocument(EMPTY_RICH_DOCUMENT), position,
  };
}
function documentWithInlineField(fieldId: string, label: string, prefix = ""): RichDocument {
  const content: Array<Record<string, unknown>> = [];
  if (prefix !== "") content.push({ type: "text", text: prefix });
  content.push({ type: "templateFieldInline", attrs: { fieldId, label } });
  return { type: "doc", content: [{ type: "paragraph", content }] };
}
function draftFromTemplate(template: CardTemplate): TemplateDraft {
  return {
    id: template.id, revision: template.revision, name: template.name, description: template.description,
    fields: template.fields.map((field) => ({ id: field.id, key: field.id, name: field.name, description: field.description, position: field.position })),
    rules: template.rules.map((rule) => ({ id: rule.id, key: rule.id, name: rule.name, frontDoc: cloneDocument(rule.frontDoc), backDoc: cloneDocument(rule.backDoc), position: rule.position })),
    assets: template.assets,
  };
}
function validateDraft(draft: TemplateDraft): string | null {
  if (draft.name.trim() === "") return "Enter a template name.";
  if (draft.fields.some((field) => field.name.trim() === "")) return "Give every field a name.";
  const normalizedFields = draft.fields.map((field) => field.name.trim().replace(/\s+/gu, " ").toLocaleLowerCase());
  if (new Set(normalizedFields).size !== normalizedFields.length) return "Field names must be unique.";
  if (draft.rules.some((rule) => rule.name.trim() === "")) return "Give every generated card a name.";
  return null;
}
function updateField(setDraft: React.Dispatch<React.SetStateAction<TemplateDraft | null>>, key: string, patch: Partial<FieldDraft>): void {
  setDraft((current) => current ? { ...current, fields: current.fields.map((field) => field.key === key ? { ...field, ...patch } : field) } : current);
}
function updateRule(setDraft: React.Dispatch<React.SetStateAction<TemplateDraft | null>>, key: string, patch: Partial<RuleDraft>): void {
  setDraft((current) => current ? { ...current, rules: current.rules.map((rule) => rule.key === key ? { ...rule, ...patch } : rule) } : current);
}
function createEntityId(): string { return crypto.randomUUID().replaceAll("-", ""); }

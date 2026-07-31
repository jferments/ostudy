import { useEffect, useMemo, useRef, useState } from "react";

import { Modal } from "../components/Modal";
import { RichTextEditor } from "../editor/RichTextEditor";
import { RichTextView } from "../editor/RichTextView";
import {
  EMPTY_RICH_DOCUMENT,
  cloneDocument,
  isDocumentEmpty,
} from "../editor/richText";
import {
  deleteCardTemplate,
  getErrorCode,
  getErrorMessage,
  listCardTemplates,
  saveAdvancedNote,
  saveCardTemplate,
  type Asset,
  type CardTemplate,
  type Deck,
  type NoteEditor,
  type NoteMode,
  type RichDocument,
  type SaveCardTemplateInput,
  type TemplateCardSpec,
} from "../ipc";

interface CardEditorScreenProps {
  deck: Deck;
  note: NoteEditor | null;
  onCancel: () => void;
  onSaved: (note: NoteEditor, continueAdding: boolean) => void | Promise<void>;
  onDirtyChange: (dirty: boolean) => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

const MODE_LABELS: Record<NoteMode, string> = {
  basic: "Basic",
  reverse: "Reverse",
  fill_blank: "Fill in the blank",
  template: "Template",
};

export function CardEditorScreen({
  deck,
  note,
  onCancel,
  onSaved,
  onDirtyChange,
  showToast,
}: CardEditorScreenProps) {
  const [mode, setMode] = useState<NoteMode>(note?.mode ?? "basic");
  const [frontDoc, setFrontDoc] = useState<RichDocument>(() =>
    note ? cloneDocument(note.frontDoc) : cloneDocument(EMPTY_RICH_DOCUMENT),
  );
  const [backDoc, setBackDoc] = useState<RichDocument>(() =>
    note ? cloneDocument(note.backDoc) : cloneDocument(EMPTY_RICH_DOCUMENT),
  );
  const [frontText, setFrontText] = useState(note?.frontText ?? "");
  const [backText, setBackText] = useState(note?.backText ?? "");
  const [tagsText, setTagsText] = useState(note?.tags.join(", ") ?? "");
  const [fillBlankText, setFillBlankText] = useState(note?.fillBlankText ?? "");
  const [templateId, setTemplateId] = useState<string | null>(note?.templateId ?? null);
  const [fields, setFields] = useState<Record<string, string>>(note?.fields ?? {});
  const [assets, setAssets] = useState<Asset[]>(note?.assets ?? []);
  const [templates, setTemplates] = useState<CardTemplate[]>([]);
  const [isLoadingTemplates, setIsLoadingTemplates] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [previewAnswer, setPreviewAnswer] = useState(false);
  const [previewOrdinal, setPreviewOrdinal] = useState(0);
  const [editorGeneration, setEditorGeneration] = useState(0);
  const [templateBuilder, setTemplateBuilder] = useState<CardTemplate | "new" | null>(null);
  const savingRef = useRef(false);

  const selectedTemplate = useMemo(
    () => templates.find((template) => template.id === templateId) ?? null,
    [templateId, templates],
  );
  const tags = useMemo(
    () => tagsText.split(",").map((tag) => tag.trim()).filter(Boolean),
    [tagsText],
  );
  const assetIds = useMemo(() => new Set(assets.map((asset) => asset.id)), [assets]);
  const fillAnswers = useMemo(() => parseFillAnswers(fillBlankText), [fillBlankText]);
  const richSidesComplete =
    !isDocumentEmpty(frontText, frontDoc) && !isDocumentEmpty(backText, backDoc);
  const templateComplete =
    selectedTemplate !== null &&
    selectedTemplate.fields.every((field) => Boolean(fields[field]?.trim()));
  const canSave =
    !isSaving &&
    !isLoadingTemplates &&
    (mode === "fill_blank"
      ? fillAnswers.length > 0
      : mode === "template"
        ? templateComplete
        : richSidesComplete);

  useEffect(() => {
    let current = true;
    void listCardTemplates()
      .then((loaded) => {
        if (!current) return;
        setTemplates(loaded);
        setTemplateId((value) => value ?? loaded[0]?.id ?? null);
      })
      .catch((error: unknown) => setErrorMessage(getErrorMessage(error)))
      .finally(() => {
        if (current) setIsLoadingTemplates(false);
      });
    return () => {
      current = false;
    };
  }, []);

  useEffect(() => {
    onDirtyChange(dirty);
    return () => onDirtyChange(false);
  }, [dirty, onDirtyChange]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent): void {
      if (!(event.ctrlKey || event.metaKey)) return;
      if (event.key.toLowerCase() === "s") {
        event.preventDefault();
        if (canSave) void save(false);
      } else if (event.key === "Enter" && note === null) {
        event.preventDefault();
        if (canSave) void save(true);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  function markDirty(): void {
    if (savingRef.current) return;
    setDirty(true);
    setErrorMessage(null);
  }

  function changeMode(next: NoteMode): void {
    setMode(next);
    setPreviewAnswer(false);
    setPreviewOrdinal(0);
    markDirty();
  }

  function addAsset(asset: Asset): void {
    if (!assetIds.has(asset.id)) setAssets((current) => [...current, asset]);
    markDirty();
  }

  async function persist(allowDuplicate: boolean): Promise<NoteEditor> {
    return saveAdvancedNote({
      noteId: note?.noteId,
      revision: note?.revision,
      deckId: deck.id,
      mode,
      frontDoc,
      backDoc,
      frontText,
      backText,
      tags,
      createReverse: mode === "reverse",
      fillBlankText: mode === "fill_blank" ? fillBlankText : null,
      templateId: mode === "template" ? templateId : null,
      fields: mode === "template" ? fields : {},
      allowDuplicate,
    });
  }

  async function save(continueAdding: boolean): Promise<void> {
    if (!canSave || savingRef.current) return;
    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      let saved: NoteEditor;
      try {
        saved = await persist(false);
      } catch (error: unknown) {
        if (getErrorCode(error) !== "duplicate_card") throw error;
        const intentional = window.confirm(
          `${getErrorMessage(error)}\n\nSave this duplicate anyway?`,
        );
        if (!intentional) return;
        saved = await persist(true);
      }
      setDirty(false);
      await onSaved(saved, continueAdding && note === null);
      if (continueAdding && note === null) resetForAnother();
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      savingRef.current = false;
      setIsSaving(false);
    }
  }

  function resetForAnother(): void {
    setMode("basic");
    setFrontDoc(cloneDocument(EMPTY_RICH_DOCUMENT));
    setBackDoc(cloneDocument(EMPTY_RICH_DOCUMENT));
    setFrontText("");
    setBackText("");
    setTagsText("");
    setFillBlankText("");
    setFields({});
    setAssets([]);
    setPreviewAnswer(false);
    setPreviewOrdinal(0);
    setEditorGeneration((value) => value + 1);
    setDirty(false);
  }

  function cancel(): void {
    if (isSaving) return;
    if (
      dirty &&
      !window.confirm("Discard the unsaved changes to this card?")
    ) {
      return;
    }
    onCancel();
  }

  async function removeSelectedTemplate(): Promise<void> {
    if (selectedTemplate === null || selectedTemplate.isBuiltin) return;
    if (!window.confirm(`Delete the “${selectedTemplate.name}” template? Templates that are used by cards cannot be deleted.`)) {
      return;
    }
    try {
      await deleteCardTemplate(selectedTemplate.id);
      const loaded = await listCardTemplates();
      setTemplates(loaded);
      setTemplateId(loaded[0]?.id ?? null);
      setFields({});
      showToast("success", "Card template deleted.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    }
  }

  const generatedCount =
    mode === "reverse"
      ? 2
      : mode === "fill_blank"
        ? fillAnswers.length
        : mode === "template"
          ? selectedTemplate?.cards.length ?? 0
          : 1;

  return (
    <div className="card-editor-screen" aria-busy={isSaving}>
      <header className="screen-header card-editor-header">
        <div>
          <button type="button" className="back-button" disabled={isSaving} onClick={cancel}>
            ← {deck.name}
          </button>
          <h1>{note ? "Edit flashcard" : "Add flashcard"}</h1>
          <p>{generatedCount} review {generatedCount === 1 ? "card" : "cards"} will be generated.</p>
        </div>
        <div className="header-actions wrap-actions">
          <button
            type="button"
            className="secondary-button"
            disabled={isSaving}
            aria-pressed={previewOpen}
            onClick={() => setPreviewOpen((value) => !value)}
          >
            {previewOpen ? "Hide preview" : "Preview"}
          </button>
          <button type="button" className="secondary-button" disabled={isSaving} onClick={cancel}>
            Cancel
          </button>
          {note === null ? (
            <button type="button" className="secondary-button" disabled={!canSave} onClick={() => void save(true)}>
              Save + add another
            </button>
          ) : null}
          <button type="button" className="primary-button" disabled={!canSave} onClick={() => void save(false)}>
            {isSaving ? "Saving…" : "Save card"}
          </button>
        </div>
      </header>

      {errorMessage ? <div className="inline-alert error-alert" role="alert">{errorMessage}</div> : null}

      <section className="card-mode-panel" aria-labelledby="card-type-heading">
        <div className="section-heading">
          <div>
            <h2 id="card-type-heading">Card type</h2>
            <p>Choose how one note becomes one or more review cards.</p>
          </div>
        </div>
        <div className="card-mode-grid" role="radiogroup" aria-label="Card type">
          {(Object.keys(MODE_LABELS) as NoteMode[]).map((value) => (
            <button
              key={value}
              type="button"
              role="radio"
              aria-checked={mode === value}
              className={mode === value ? "card-mode-option selected" : "card-mode-option"}
              disabled={isSaving}
              onClick={() => changeMode(value)}
            >
              <strong>{MODE_LABELS[value]}</strong>
              <span>{modeDescription(value)}</span>
            </button>
          ))}
        </div>
      </section>

      <div className={previewOpen ? "card-editor-layout with-preview" : "card-editor-layout"}>
        <div className="card-editor-main">
          {mode === "basic" || mode === "reverse" ? (
            <>
              <RichTextEditor
                key={`front-${editorGeneration}`}
                label="Question"
                description="Write what you want to be prompted with."
                document={frontDoc}
                assets={assets}
                disabled={isSaving}
                onChange={(document, text) => {
                  setFrontDoc(document);
                  setFrontText(text);
                  markDirty();
                }}
                onAssetImported={addAsset}
              />
              <RichTextEditor
                key={`back-${editorGeneration}`}
                label="Answer"
                description="Write the answer or explanation."
                document={backDoc}
                assets={assets}
                disabled={isSaving}
                onChange={(document, text) => {
                  setBackDoc(document);
                  setBackText(text);
                  markDirty();
                }}
                onAssetImported={addAsset}
              />
            </>
          ) : null}

          {mode === "fill_blank" ? (
            <section className="plain-editor-card">
              <div className="editor-field-heading">
                <div>
                  <h3>Fill-in-the-blank text</h3>
                  <p>
                    Put double square brackets around each answer, for example:
                    <code> Warfarin is a [[vitamin K antagonist]].</code>
                  </p>
                </div>
              </div>
              <textarea
                autoFocus
                value={fillBlankText}
                rows={12}
                maxLength={20_000}
                disabled={isSaving}
                onChange={(event) => {
                  setFillBlankText(event.currentTarget.value);
                  setPreviewOrdinal(0);
                  markDirty();
                }}
                aria-describedby="fill-blank-help"
                placeholder="The [[left ventricle]] pumps blood into the [[aorta]]."
              />
              <p id="fill-blank-help" className="field-help">
                {fillAnswers.length === 0
                  ? "Mark at least one answer with [[double brackets]]."
                  : `${fillAnswers.length} separate review ${fillAnswers.length === 1 ? "card" : "cards"} will be created.`}
              </p>
            </section>
          ) : null}

          {mode === "template" ? (
            <section className="plain-editor-card template-entry-card">
              <div className="editor-field-heading template-heading-actions">
                <div>
                  <h3>Card template</h3>
                  <p>Fields are inserted as literal text; template content is never executed.</p>
                </div>
                <div className="header-actions">
                  <button type="button" className="small-ghost-button" onClick={() => setTemplateBuilder("new")}>New template</button>
                  {selectedTemplate && !selectedTemplate.isBuiltin ? (
                    <>
                      <button type="button" className="small-ghost-button" onClick={() => setTemplateBuilder(selectedTemplate)}>Edit</button>
                      <button type="button" className="small-ghost-button danger-text" onClick={() => void removeSelectedTemplate()}>Delete</button>
                    </>
                  ) : null}
                </div>
              </div>
              <label className="plain-editor-field">
                <span>Template</span>
                <select
                  value={templateId ?? ""}
                  disabled={isLoadingTemplates || isSaving}
                  onChange={(event) => {
                    setTemplateId(event.currentTarget.value || null);
                    setFields({});
                    setPreviewOrdinal(0);
                    markDirty();
                  }}
                >
                  {templates.map((template) => (
                    <option key={template.id} value={template.id}>
                      {template.name}{template.isBuiltin ? " (built in)" : ""}
                    </option>
                  ))}
                </select>
              </label>
              {selectedTemplate ? (
                <>
                  {selectedTemplate.description ? <p className="muted-copy">{selectedTemplate.description}</p> : null}
                  <div className="template-field-grid">
                    {selectedTemplate.fields.map((field) => (
                      <label key={field}>
                        <span>{field}</span>
                        <textarea
                          value={fields[field] ?? ""}
                          rows={field.length > 25 ? 3 : 2}
                          disabled={isSaving}
                          onChange={(event) => {
                            const value = event.currentTarget.value;
                            setFields((current) => ({ ...current, [field]: value }));
                            markDirty();
                          }}
                          required
                        />
                      </label>
                    ))}
                  </div>
                  <p className="field-help">
                    Generates {selectedTemplate.cards.length} focused review {selectedTemplate.cards.length === 1 ? "card" : "cards"}.
                  </p>
                </>
              ) : (
                <p className="muted-copy">Create or select a template.</p>
              )}
            </section>
          ) : null}

          <label className="plain-editor-field compact-plain-field">
            <span>
              <strong>Tags</strong>
              <small>Separate tags with commas. Tags are searchable and filterable.</small>
            </span>
            <input
              value={tagsText}
              disabled={isSaving}
              maxLength={8_000}
              onChange={(event) => {
                setTagsText(event.currentTarget.value);
                markDirty();
              }}
              placeholder="cardiology, exam 2, high priority"
            />
          </label>

          <section className="security-note" aria-label="Shared deck safety">
            <strong>Safe shared-deck format</strong>
            <p>
              ostudy stores validated structured text and local image references—not executable HTML, JavaScript, SQL, or template code.
            </p>
          </section>
        </div>

        {previewOpen ? (
          <aside className="editor-preview-panel" aria-label="Card preview">
            <div className="preview-panel-heading">
              <div>
                <span className="eyebrow">LIVE PREVIEW</span>
                <h2>{previewAnswer ? "Answer" : "Question"}</h2>
              </div>
              <button type="button" className="small-primary-button" onClick={() => setPreviewAnswer((value) => !value)}>
                {previewAnswer ? "Show question" : "Reveal answer"}
              </button>
            </div>
            <div className="flashcard-preview-surface">
              {mode === "basic" || mode === "reverse" ? (
                <RichTextView
                  document={previewDocument(
                    mode,
                    previewOrdinal,
                    previewAnswer,
                    frontDoc,
                    backDoc,
                  )}
                  assets={assets}
                  onLinkError={setErrorMessage}
                />
              ) : (
                <p>{previewPlainText(mode, previewAnswer, previewOrdinal, fillBlankText, selectedTemplate, fields)}</p>
              )}
            </div>
            {generatedCount > 1 ? (
              <label className="preview-card-selector">
                <span>Generated card</span>
                <select value={Math.min(previewOrdinal, generatedCount - 1)} onChange={(event) => { setPreviewOrdinal(Number(event.currentTarget.value)); setPreviewAnswer(false); }}>
                  {Array.from({ length: generatedCount }, (_value, index) => (
                    <option key={index} value={index}>{previewCardName(mode, index, selectedTemplate)}</option>
                  ))}
                </select>
              </label>
            ) : null}
            <p className="preview-card-count">Generates {generatedCount} card{generatedCount === 1 ? "" : "s"}</p>
          </aside>
        ) : null}
      </div>

      {templateBuilder ? (
        <TemplateBuilderModal
          template={templateBuilder === "new" ? null : templateBuilder}
          onClose={() => setTemplateBuilder(null)}
          onSaved={(saved) => {
            setTemplates((current) => {
              const without = current.filter((template) => template.id !== saved.id);
              return [...without, saved].sort((a, b) => a.name.localeCompare(b.name));
            });
            setTemplateId(saved.id);
            setFields((current) => Object.fromEntries(saved.fields.map((field) => [field, current[field] ?? ""])));
            setTemplateBuilder(null);
            markDirty();
            showToast("success", templateBuilder === "new" ? "Card template created." : "Card template updated.");
          }}
        />
      ) : null}
    </div>
  );
}

function modeDescription(mode: NoteMode): string {
  switch (mode) {
    case "basic":
      return "One question and answer";
    case "reverse":
      return "Study both directions";
    case "fill_blank":
      return "Hide marked words or phrases";
    case "template":
      return "Generate cards from named fields";
  }
}

function parseFillAnswers(source: string): string[] {
  const answers: string[] = [];
  for (const match of source.matchAll(/\[\[([^\[\]]+)\]\]/gu)) {
    const answer = match[1]?.trim() ?? "";
    if (answer !== "") answers.push(answer);
  }
  return answers;
}

function renderFillQuestion(source: string, target: number): string {
  let ordinal = 0;
  return source.replace(/\[\[([^\[\]]+)\]\]/gu, (_match, answer: string) => {
    const current = ordinal;
    ordinal += 1;
    return current === target ? "_____" : answer.trim();
  });
}

function renderPattern(pattern: string, fields: Record<string, string>): string {
  return pattern.replace(/\{\{([^{}]+)\}\}/gu, (_match, name: string) => {
    const normalized = name.trim().replace(/\s+/gu, " ").toLocaleLowerCase();
    return fields[normalized]?.trim() || `{{${normalized}}}`;
  });
}

function previewDocument(
  mode: NoteMode,
  ordinal: number,
  answer: boolean,
  front: RichDocument,
  back: RichDocument,
): RichDocument {
  const reversed = mode === "reverse" && ordinal === 1;
  if (reversed) return answer ? front : back;
  return answer ? back : front;
}

function previewPlainText(
  mode: NoteMode,
  answer: boolean,
  ordinal: number,
  fillBlankText: string,
  template: CardTemplate | null,
  fields: Record<string, string>,
): string {
  if (mode === "fill_blank") {
    const answers = parseFillAnswers(fillBlankText);
    if (answers.length === 0) return "Mark an answer with [[double brackets]] to preview it.";
    const target = Math.min(ordinal, answers.length - 1);
    return answer
      ? fillBlankText.replace(/\[\[([^\[\]]+)\]\]/gu, (_match, value: string) => value.trim())
      : renderFillQuestion(fillBlankText, target);
  }
  if (mode === "template" && template) {
    const spec = template.cards[Math.min(ordinal, template.cards.length - 1)];
    if (!spec) return "Add a generated card to this template.";
    return renderPattern(answer ? spec.back : spec.front, fields);
  }
  return "";
}

function previewCardName(mode: NoteMode, ordinal: number, template: CardTemplate | null): string {
  if (mode === "reverse") return ordinal === 0 ? "Forward" : "Reverse";
  if (mode === "basic") return ordinal === 0 ? "Forward" : "Reverse";
  if (mode === "fill_blank") return `Blank ${ordinal + 1}`;
  if (mode === "template") return template?.cards[ordinal]?.name ?? `Card ${ordinal + 1}`;
  return "Forward";
}

interface TemplateBuilderModalProps {
  template: CardTemplate | null;
  onClose: () => void;
  onSaved: (template: CardTemplate) => void;
}

function TemplateBuilderModal({ template, onClose, onSaved }: TemplateBuilderModalProps) {
  const [name, setName] = useState(template?.name ?? "");
  const [description, setDescription] = useState(template?.description ?? "");
  const [fieldsText, setFieldsText] = useState(template?.fields.join("\n") ?? "term\ndefinition");
  const [cards, setCards] = useState<TemplateCardSpec[]>(
    template?.cards ?? [{ name: "Definition", front: "What is {{term}}?", back: "{{definition}}" }],
  );
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  const fields = useMemo(
    () => fieldsText.split("\n").map((field) => field.trim()).filter(Boolean),
    [fieldsText],
  );
  const canSave =
    !isSaving &&
    name.trim() !== "" &&
    fields.length > 0 &&
    cards.length > 0 &&
    cards.every((card) => card.name.trim() && card.front.trim() && card.back.trim());

  async function persist(): Promise<void> {
    if (!canSave) return;
    setIsSaving(true);
    setErrorMessage(null);
    const input: SaveCardTemplateInput = {
      templateId: template?.id,
      revision: template?.revision,
      name,
      description,
      fields,
      cards,
    };
    try {
      onSaved(await saveCardTemplate(input));
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <Modal
      title={template ? "Edit card template" : "Create card template"}
      description="Templates use literal text and exact {{field name}} placeholders. No code is executed."
      onClose={() => { if (!isSaving) onClose(); }}
      width="large"
    >
      <div className="template-builder">
        {errorMessage ? <div className="inline-alert error-alert" role="alert">{errorMessage}</div> : null}
        <div className="form-grid-two">
          <label>
            <span>Template name</span>
            <input autoFocus value={name} maxLength={120} disabled={isSaving} onChange={(event) => setName(event.currentTarget.value)} />
          </label>
          <label>
            <span>Description</span>
            <input value={description} maxLength={2000} disabled={isSaving} onChange={(event) => setDescription(event.currentTarget.value)} />
          </label>
        </div>
        <label>
          <span>Fields <small>one field name per line</small></span>
          <textarea value={fieldsText} rows={5} disabled={isSaving} onChange={(event) => setFieldsText(event.currentTarget.value)} />
        </label>
        <fieldset>
          <legend>Generated cards</legend>
          {cards.map((card, index) => (
            <div className="template-rule-builder" key={index}>
              <input
                aria-label={`Generated card ${index + 1} name`}
                value={card.name}
                disabled={isSaving}
                placeholder="Card name"
                onChange={(event) => setCards((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, name: event.currentTarget.value } : item))}
              />
              <textarea
                aria-label={`Generated card ${index + 1} question`}
                value={card.front}
                rows={2}
                disabled={isSaving}
                placeholder="Question using {{field name}}"
                onChange={(event) => setCards((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, front: event.currentTarget.value } : item))}
              />
              <textarea
                aria-label={`Generated card ${index + 1} answer`}
                value={card.back}
                rows={2}
                disabled={isSaving}
                placeholder="Answer using {{field name}}"
                onChange={(event) => setCards((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, back: event.currentTarget.value } : item))}
              />
              <button type="button" className="small-ghost-button" disabled={isSaving || cards.length === 1} onClick={() => setCards((current) => current.filter((_item, itemIndex) => itemIndex !== index))}>
                Remove
              </button>
            </div>
          ))}
          <button type="button" className="small-ghost-button" disabled={isSaving || cards.length >= 64} onClick={() => setCards((current) => [...current, { name: "", front: "", back: "" }])}>
            + Add generated card
          </button>
        </fieldset>
        <div className="modal-actions">
          <button type="button" className="secondary-button" disabled={isSaving} onClick={onClose}>Cancel</button>
          <button type="button" className="primary-button" disabled={!canSave} onClick={() => void persist()}>
            {isSaving ? "Saving…" : template ? "Save template" : "Create template"}
          </button>
        </div>
      </div>
    </Modal>
  );
}

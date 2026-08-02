import { useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent, ReactNode } from "react";

import { Modal } from "../../components/Modal";
import { TagInput } from "../../components/TagInput";
import { RichTextEditor } from "../../editor/RichTextEditor";
import { RichTextView } from "../../editor/RichTextView";
import {
  createOrganizedCard,
  findDuplicateCards,
  generateCardsFromTemplate,
  getErrorCode,
  getErrorMessage,
  getOrganizedCard,
  listCardTemplates,
  listDecks,
  listTags,
  updateOrganizedCard,
  type Asset,
  type CardTemplate,
  type CardType,
  type ClozeMode,
  type Deck,
  type Flashcard,
  type OrganizedCard,
  type RichDocument,
  type TagSummary,
  type TemplateValueInput,
} from "../../ipc";
import { ClozeEditor } from "./ClozeEditor";
import { parseClozeMarkup, renderClozeQuestion } from "./cloze";

export interface CardEditorSaveSummary {
  source: "card" | "template";
  createdCount: number;
  skippedDuplicates: number;
}

interface CardEditorScreenProps {
  deck: Deck;
  decks?: Deck[];
  card: Flashcard | OrganizedCard | null;
  onCancel: () => void;
  onSaved: (
    card: OrganizedCard | null,
    continueAdding: boolean,
    summary: CardEditorSaveSummary,
  ) => void | Promise<void>;
  onDirtyChange: (dirty: boolean) => void;
  onMoveCopy?: () => void;
}

type AuthoringChoice = "basic" | "cloze" | `template:${string}`;

const EMPTY_DOCUMENT: RichDocument = {
  type: "doc",
  content: [{ type: "paragraph" }],
};

function emptyDocument(): RichDocument {
  return structuredClone(EMPTY_DOCUMENT);
}

export function CardEditorScreen({
  deck,
  decks: providedDecks,
  card,
  onCancel,
  onSaved,
  onDirtyChange,
  onMoveCopy,
}: CardEditorScreenProps) {
  const [availableDecks, setAvailableDecks] = useState<Deck[]>(providedDecks ?? [deck]);
  const [templates, setTemplates] = useState<CardTemplate[]>([]);
  const [tagSuggestions, setTagSuggestions] = useState<TagSummary[]>([]);
  const [sourceCard, setSourceCard] = useState<OrganizedCard | null>(
    isOrganizedCard(card) ? card : null,
  );
  const [isLoading, setIsLoading] = useState(card !== null && !isOrganizedCard(card));
  const [isSaving, setIsSaving] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [duplicateOpen, setDuplicateOpen] = useState(false);
  const [duplicateMessage, setDuplicateMessage] = useState("");

  const [deckId, setDeckId] = useState(sourceCard?.deckId ?? deck.id);
  const [choice, setChoice] = useState<AuthoringChoice>(
    sourceCard?.cardType ?? "basic",
  );
  const [frontDoc, setFrontDoc] = useState<RichDocument>(
    sourceCard?.frontDoc ?? emptyDocument(),
  );
  const [backDoc, setBackDoc] = useState<RichDocument>(
    sourceCard?.backDoc ?? emptyDocument(),
  );
  const [frontText, setFrontText] = useState(sourceCard?.frontText ?? "");
  const [backText, setBackText] = useState(sourceCard?.backText ?? "");
  const [assets, setAssets] = useState<Asset[]>(sourceCard?.assets ?? []);
  const [reverseEnabled, setReverseEnabled] = useState(
    sourceCard?.reverseEnabled ?? false,
  );
  const [clozeText, setClozeText] = useState(sourceCard?.clozeText ?? "");
  const [clozeMode, setClozeMode] = useState<ClozeMode>(
    sourceCard?.clozeMode ?? "one_at_a_time",
  );
  const [templateValues, setTemplateValues] = useState<TemplateValueInput[]>([]);
  const [tags, setTags] = useState(sourceCard?.tags ?? []);
  const initializedNote = useRef<string | null>(
    sourceCard?.noteId ?? (card === null ? "new" : null),
  );

  const editing = card !== null;
  const selectedTemplateId = choice.startsWith("template:")
    ? choice.slice("template:".length)
    : null;
  const selectedTemplate = useMemo(
    () => templates.find((template) => template.id === selectedTemplateId) ?? null,
    [selectedTemplateId, templates],
  );
  const cardType: CardType = choice === "cloze" ? "cloze" : "basic";
  const isTemplateMode = selectedTemplate !== null && !editing;
  const parsedCloze = useMemo(() => parseClozeMarkup(clozeText), [clozeText]);
  const generatedCount = useMemo(() => {
    if (isTemplateMode) return selectedTemplate.rules.length;
    if (cardType === "basic") return reverseEnabled ? 2 : 1;
    return clozeMode === "all_together"
      ? parsedCloze.groups.length > 0
        ? 1
        : 0
      : parsedCloze.groups.length;
  }, [cardType, clozeMode, isTemplateMode, parsedCloze.groups.length, reverseEnabled, selectedTemplate]);

  useEffect(() => {
    let active = true;
    void Promise.all([
      providedDecks ? Promise.resolve(providedDecks) : listDecks(),
      listCardTemplates(),
      listTags(),
    ])
      .then(([loadedDecks, loadedTemplates, loadedTags]) => {
        if (!active) return;
        setAvailableDecks(loadedDecks);
        setTemplates(loadedTemplates);
        setTagSuggestions(loadedTags);
      })
      .catch((error: unknown) => {
        if (active) setLoadError(getErrorMessage(error));
      });
    return () => {
      active = false;
    };
  }, [providedDecks]);

  useEffect(() => {
    if (card === null || isOrganizedCard(card)) return;
    let active = true;
    setIsLoading(true);
    void getOrganizedCard(card.noteId)
      .then((loaded) => {
        if (active) setSourceCard(loaded);
      })
      .catch((error: unknown) => {
        if (active) setLoadError(getErrorMessage(error));
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [card]);

  useEffect(() => {
    if (!sourceCard || initializedNote.current === sourceCard.noteId) return;
    initializedNote.current = sourceCard.noteId;
    setDeckId(sourceCard.deckId);
    setChoice(sourceCard.cardType);
    setFrontDoc(sourceCard.frontDoc);
    setBackDoc(sourceCard.backDoc);
    setFrontText(sourceCard.frontText);
    setBackText(sourceCard.backText);
    setAssets(sourceCard.assets);
    setReverseEnabled(sourceCard.reverseEnabled);
    setClozeText(sourceCard.clozeText);
    setClozeMode(sourceCard.clozeMode);
    setTags(sourceCard.tags);
    onDirtyChange(false);
  }, [onDirtyChange, sourceCard]);

  useEffect(() => {
    if (!selectedTemplate || editing) return;
    setTemplateValues((current) =>
      selectedTemplate.fields.map((field) => ({
        fieldId: field.id,
        value: current.find((value) => value.fieldId === field.id)?.value ?? "",
      })),
    );
  }, [editing, selectedTemplate]);

  function markDirty(): void {
    setSaveError(null);
    onDirtyChange(true);
  }

  function selectAuthoringChoice(value: string): void {
    if (editing) return;
    if (value === "basic" || value === "cloze" || value.startsWith("template:")) {
      setChoice(value as AuthoringChoice);
      markDirty();
    }
  }

  function clientValidationMessage(): string | null {
    if (isTemplateMode) {
      const missing = selectedTemplate.fields.find(
        (field) =>
          (templateValues.find((value) => value.fieldId === field.id)?.value ?? "").trim() === "",
      );
      return missing ? `Enter a value for ${missing.name}.` : null;
    }
    if (cardType === "basic") {
      if (frontText.trim() === "" && assets.length === 0) return "Add a question.";
      if (backText.trim() === "" && assets.length === 0) return "Add an answer.";
      return null;
    }
    if (!parsedCloze.valid) return "Fix the incomplete blank marker before saving.";
    if (parsedCloze.groups.length === 0) return "Mark at least one word or phrase as a blank.";
    return null;
  }

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    await attemptSave(false, false);
  }

  async function attemptSave(
    allowDuplicate: boolean,
    continueAdding: boolean,
  ): Promise<void> {
    if (isSaving) return;
    const validation = clientValidationMessage();
    if (validation) {
      setSaveError(validation);
      return;
    }

    setIsSaving(true);
    setSaveError(null);
    try {
      if (isTemplateMode) {
        const result = await generateCardsFromTemplate({
          deckId,
          templateId: selectedTemplate.id,
          values: templateValues,
          tags,
        });
        if (result.createdCount === 0) {
          setSaveError(
            result.skippedDuplicates > 0
              ? "Every card produced by this entry already exists in the selected deck."
              : "This template did not produce any cards.",
          );
          return;
        }
        const primary = result.createdNoteIds[0]
          ? await getOrganizedCard(result.createdNoteIds[0])
          : null;
        onDirtyChange(false);
        await onSaved(primary, continueAdding, {
          source: "template",
          createdCount: result.createdCount,
          skippedDuplicates: result.skippedDuplicates,
        });
        if (continueAdding) resetForAnother();
        return;
      }

      if (!allowDuplicate && cardType === "basic") {
        const duplicates = await findDuplicateCards({
          deckId,
          frontText,
          backText,
          excludeNoteId: sourceCard?.noteId,
        });
        if (duplicates.length > 0) {
          setDuplicateMessage(
            duplicates.length === 1
              ? "An identical question and answer already exist in this deck."
              : `${duplicates.length} identical cards already exist in this deck.`,
          );
          setDuplicateOpen(true);
          return;
        }
      }

      const common = {
        deckId,
        cardType,
        reverseEnabled: cardType === "basic" && reverseEnabled,
        clozeText: cardType === "cloze" ? clozeText : "",
        clozeMode,
        frontDoc: cardType === "basic" ? frontDoc : emptyDocument(),
        backDoc: cardType === "basic" ? backDoc : emptyDocument(),
        frontText: cardType === "basic" ? frontText : "",
        backText: cardType === "basic" ? backText : "",
        tags,
        allowDuplicate,
      } as const;
      const saved = sourceCard
        ? await updateOrganizedCard({
            ...common,
            noteId: sourceCard.noteId,
            revision: sourceCard.revision,
          })
        : await createOrganizedCard(common);

      onDirtyChange(false);
      await onSaved(saved, continueAdding && !editing, {
        source: "card",
        createdCount: saved.generatedCardCount,
        skippedDuplicates: 0,
      });
      if (continueAdding && !editing) resetForAnother();
    } catch (error: unknown) {
      if (!allowDuplicate && getErrorCode(error) === "duplicate_card") {
        setDuplicateMessage(
          "A note with the same source content already exists in this deck.",
        );
        setDuplicateOpen(true);
      } else {
        setSaveError(getErrorMessage(error));
      }
    } finally {
      setIsSaving(false);
    }
  }

  function resetForAnother(): void {
    setSourceCard(null);
    initializedNote.current = "new";
    if (isTemplateMode) {
      setTemplateValues((current) => current.map((value) => ({ ...value, value: "" })));
    } else if (cardType === "cloze") {
      setClozeText("");
    } else {
      setFrontDoc(emptyDocument());
      setBackDoc(emptyDocument());
      setFrontText("");
      setBackText("");
      setAssets([]);
    }
    setDuplicateOpen(false);
    setSaveError(null);
    onDirtyChange(false);
  }

  if (isLoading) {
    return (
      <div className="screen">
        <div className="inline-progress">Loading card source…</div>
      </div>
    );
  }

  const saveLabel = isTemplateMode
    ? `Create ${generatedCount} ${generatedCount === 1 ? "card" : "cards"}`
    : "Save card";

  return (
    <form
      className="screen card-editor-screen typed-card-editor"
      onSubmit={(event) => void submit(event)}
    >
      <header className="screen-header card-editor-header">
        <div>
          <button type="button" className="back-button" onClick={onCancel}>
            ← Back
          </button>
          <h1>{editing ? "Edit flashcard" : "Add flashcard"}</h1>
          <p>
            {isTemplateMode
              ? `${selectedTemplate.name} will create ${generatedCount} ordinary Basic ${generatedCount === 1 ? "card" : "cards"} in ${availableDecks.find((candidate) => candidate.id === deckId)?.name ?? "the selected deck"}.`
              : `${generatedCount} independently scheduled ${generatedCount === 1 ? "card" : "cards"} will be stored for this note.`}
          </p>
        </div>
        <div className="header-actions wrap-actions">
          {editing && onMoveCopy ? (
            <button type="button" className="secondary-button" onClick={onMoveCopy}>
              Move or copy
            </button>
          ) : null}
          <button
            type="button"
            className="secondary-button"
            onClick={onCancel}
            disabled={isSaving}
          >
            Cancel
          </button>
          {!editing ? (
            <button
              type="button"
              className="secondary-button"
              disabled={isSaving || generatedCount === 0}
              onClick={() => void attemptSave(false, true)}
            >
              {isTemplateMode ? "Create + another" : "Save + another"}
            </button>
          ) : null}
          <button
            type="submit"
            className="primary-button"
            disabled={isSaving || generatedCount === 0}
          >
            {isSaving ? "Saving…" : saveLabel}
          </button>
        </div>
      </header>

      {loadError ? <div className="inline-alert error-alert">{loadError}</div> : null}
      {saveError ? <div className="inline-alert error-alert">{saveError}</div> : null}

      <div className="typed-card-editor-layout">
        <main className="typed-card-fields">
          <section className="card-type-panel">
            <label>
              <span>Card template</span>
              <select
                aria-label="Card template"
                value={choice}
                disabled={editing || isSaving}
                onChange={(event) => selectAuthoringChoice(event.currentTarget.value)}
              >
                <option value="basic">Basic</option>
                <option value="cloze">Fill in the blank</option>
                {templates.length > 0 ? (
                  <optgroup label="Reusable entry templates">
                    {templates.map((template) => (
                      <option key={template.id} value={`template:${template.id}`}>
                        {template.name}
                      </option>
                    ))}
                  </optgroup>
                ) : null}
              </select>
            </label>
            <div className="card-type-summary">
              <strong>{choiceLabel(cardType, selectedTemplate)}</strong>
              <span>{choiceDescription(cardType, selectedTemplate)}</span>
            </div>
            {editing ? (
              <small>
                Basic versus Fill in the blank is locked after saving so generated card
                identities and review histories cannot be reassigned accidentally.
              </small>
            ) : null}
          </section>

          <label className="editor-select-control">
            <span>Deck</span>
            <select
              aria-label="Deck"
              value={deckId}
              disabled={isSaving}
              onChange={(event) => {
                setDeckId(event.currentTarget.value);
                markDirty();
              }}
            >
              {availableDecks.map((candidate) => (
                <option key={candidate.id} value={candidate.id}>
                  {candidate.name}
                </option>
              ))}
            </select>
          </label>

          {!isTemplateMode && cardType === "basic" ? (
            <>
              <RichTextEditor
                label="Question"
                document={frontDoc}
                assets={assets}
                disabled={isSaving}
                onChange={(document, plainText) => {
                  setFrontDoc(document);
                  setFrontText(plainText);
                  markDirty();
                }}
                onAssetImported={(asset) => {
                  setAssets((current) => [...current, asset]);
                  markDirty();
                }}
              />
              <RichTextEditor
                label="Answer"
                document={backDoc}
                assets={assets}
                disabled={isSaving}
                onChange={(document, plainText) => {
                  setBackDoc(document);
                  setBackText(plainText);
                  markDirty();
                }}
                onAssetImported={(asset) => {
                  setAssets((current) => [...current, asset]);
                  markDirty();
                }}
              />
              <label className="reverse-card-toggle">
                <input
                  type="checkbox"
                  checked={reverseEnabled}
                  disabled={isSaving}
                  onChange={(event) => {
                    setReverseEnabled(event.currentTarget.checked);
                    markDirty();
                  }}
                />
                <span>
                  <strong>Also create a reverse card</strong>
                  <small>
                    Use this only when either side makes sense as the prompt. The forward
                    and reverse cards keep separate schedules.
                  </small>
                </span>
              </label>
            </>
          ) : null}

          {!isTemplateMode && cardType === "cloze" ? (
            <ClozeEditor
              value={clozeText}
              mode={clozeMode}
              disabled={isSaving}
              onChange={(value) => {
                setClozeText(value);
                markDirty();
              }}
              onModeChange={(mode, value) => {
                setClozeMode(mode);
                setClozeText(value);
                markDirty();
              }}
            />
          ) : null}

          {isTemplateMode ? (
            <section
              className="template-value-fields"
              aria-label={`${selectedTemplate.name} fields`}
            >
              <header>
                <h2>{selectedTemplate.name}</h2>
                {selectedTemplate.description ? <p>{selectedTemplate.description}</p> : null}
                <small>
                  Submitting this form creates ordinary Basic cards. Those cards remain in
                  the deck even if this template is later edited or deleted.
                </small>
              </header>
              {selectedTemplate.fields.map((field) => (
                <label key={field.id}>
                  <span>{field.name}</span>
                  {field.description ? <small>{field.description}</small> : null}
                  <textarea
                    rows={field.description ? 4 : 3}
                    value={
                      templateValues.find((value) => value.fieldId === field.id)?.value ?? ""
                    }
                    disabled={isSaving}
                    onChange={(event) => {
                      const value = event.currentTarget.value;
                      setTemplateValues((current) =>
                        selectedTemplate.fields.map((candidate) => ({
                          fieldId: candidate.id,
                          value:
                            candidate.id === field.id
                              ? value
                              : current.find((item) => item.fieldId === candidate.id)?.value ?? "",
                        })),
                      );
                      markDirty();
                    }}
                  />
                </label>
              ))}
            </section>
          ) : null}

          <TagInput
            tags={tags}
            suggestions={tagSuggestions}
            disabled={isSaving}
            onChange={(next) => {
              setTags(next);
              markDirty();
            }}
          />
        </main>

        <aside className="typed-card-preview" aria-label="Generated card preview">
          <header>
            <span>Cards created</span>
            <strong>{generatedCount}</strong>
          </header>
          <CardPreview
            cardType={cardType}
            template={selectedTemplate}
            templateValues={templateValues}
            frontDoc={frontDoc}
            backDoc={backDoc}
            assets={assets}
            clozeText={clozeText}
            clozeMode={clozeMode}
            reverseEnabled={reverseEnabled}
          />
        </aside>
      </div>

      {duplicateOpen ? (
        <Modal
          title="This looks like an exact duplicate"
          description={duplicateMessage}
          width="small"
          onClose={() => setDuplicateOpen(false)}
        >
          <div className="modal-body">
            <p>Save it only when the duplicate is intentional.</p>
          </div>
          <footer className="modal-actions">
            <button
              type="button"
              className="secondary-button"
              onClick={() => setDuplicateOpen(false)}
            >
              Go back
            </button>
            <button
              type="button"
              className="primary-button"
              onClick={() => {
                setDuplicateOpen(false);
                void attemptSave(true, false);
              }}
            >
              Save duplicate anyway
            </button>
          </footer>
        </Modal>
      ) : null}
    </form>
  );
}

interface CardPreviewProps {
  cardType: CardType;
  template: CardTemplate | null;
  templateValues: TemplateValueInput[];
  frontDoc: RichDocument;
  backDoc: RichDocument;
  assets: Asset[];
  clozeText: string;
  clozeMode: ClozeMode;
  reverseEnabled: boolean;
}

function CardPreview({
  cardType,
  template,
  templateValues,
  frontDoc,
  backDoc,
  assets,
  clozeText,
  clozeMode,
  reverseEnabled,
}: CardPreviewProps) {
  if (template) {
    const valueByField = new Map(templateValues.map((value) => [value.fieldId, value.value]));
    const fieldByName = new Map(
      template.fields.map((field) => [
        normalizeField(field.name),
        valueByField.get(field.id) ?? "",
      ]),
    );
    return (
      <div className="template-rule-previews">
        {template.rules.map((rule) => (
          <article key={rule.id}>
            <strong>{rule.name}</strong>
            <PreviewSide label="Question">
              <p>{renderTemplatePattern(rule.frontPattern, fieldByName)}</p>
            </PreviewSide>
            <PreviewSide label="Answer">
              <p>{renderTemplatePattern(rule.backPattern, fieldByName)}</p>
            </PreviewSide>
          </article>
        ))}
      </div>
    );
  }
  if (cardType === "basic") {
    return (
      <div className="generated-preview-stack">
        <PreviewSide label="Forward question">
          <RichTextView document={frontDoc} assets={assets} />
        </PreviewSide>
        <PreviewSide label="Forward answer">
          <RichTextView document={backDoc} assets={assets} />
        </PreviewSide>
        {reverseEnabled ? (
          <>
            <PreviewSide label="Reverse question">
              <RichTextView document={backDoc} assets={assets} />
            </PreviewSide>
            <PreviewSide label="Reverse answer">
              <RichTextView document={frontDoc} assets={assets} />
            </PreviewSide>
          </>
        ) : null}
      </div>
    );
  }
  const parsed = parseClozeMarkup(clozeText);
  if (!parsed.valid || parsed.groups.length === 0) {
    return <p className="preview-empty">Mark a blank to preview the card.</p>;
  }
  const previewGroups = clozeMode === "all_together" ? [null] : parsed.groups;
  return (
    <div className="template-rule-previews">
      {previewGroups.map((group, index) => (
        <article key={group ?? "all"}>
          <strong>{group === null ? "All blanks" : `Blank ${group}`}</strong>
          <PreviewSide label="Question">
            <p>{renderClozeQuestion(parsed, group, clozeMode)}</p>
          </PreviewSide>
          <PreviewSide label="Answer">
            <p>{parsed.fullText}</p>
          </PreviewSide>
          {index >= 4 && previewGroups.length > 5 ? null : null}
        </article>
      ))}
    </div>
  );
}

function PreviewSide({ label, children }: { label: string; children: ReactNode }) {
  return (
    <section className="generated-preview-side">
      <span>{label}</span>
      {children}
    </section>
  );
}

function renderTemplatePattern(pattern: string, values: Map<string, string>): string {
  return pattern.replace(/\{\{([^{}]+)\}\}/gu, (_match, name: string) => {
    const value = values.get(normalizeField(name)) ?? "";
    return value.trim() === "" ? `⟦${name.trim()}⟧` : value;
  });
}

function normalizeField(value: string): string {
  return value.trim().replace(/\s+/gu, " ").toLocaleLowerCase();
}

function isOrganizedCard(value: Flashcard | OrganizedCard | null): value is OrganizedCard {
  return value !== null && "tags" in value && "reverseEnabled" in value;
}

function choiceLabel(cardType: CardType, template: CardTemplate | null): string {
  if (template) return template.name;
  return cardType === "basic" ? "Question and answer" : "Fill in the blank";
}

function choiceDescription(cardType: CardType, template: CardTemplate | null): string {
  if (template) {
    return (
      template.description ||
      "Fill one structured form to create one ordinary Basic card per template rule."
    );
  }
  if (cardType === "basic") {
    return "One forward card, with an optional independently scheduled reverse card.";
  }
  return "Mark one or more regions and choose whether they are hidden separately or together.";
}

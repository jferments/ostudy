import { useEffect, useMemo, useRef, useState } from "react";

import { TagInput } from "../components/TagInput";
import { RichTextEditor } from "../editor/RichTextEditor";
import { RichTextView } from "../editor/RichTextView";
import {
  EMPTY_RICH_DOCUMENT,
  cloneDocument,
  isDocumentEmpty,
} from "../editor/richText";
import {
  createGeneratedCard,
  findDuplicateCards,
  generateCardsFromTemplate,
  getCardTypeInfo,
  getErrorCode,
  getErrorMessage,
  getOrganizedCard,
  listCardTemplates,
  listDecks,
  listTags,
  previewTemplateCards,
  updateGeneratedCard,
  type Asset,
  type BuiltInCardType,
  type CardTemplate,
  type ClozeMode,
  type Deck,
  type DuplicateCard,
  type Flashcard,
  type GenerateTemplateCardsResult,
  type OrganizedCard,
  type RichDocument,
  type TagSummary,
  type TemplateCardPreview,
  type TemplateValueInput,
} from "../ipc";
import { MoveCopyModal } from "./cards/MoveCopyModal";
import { ClozeEditor } from "./card-editor/ClozeEditor";
import { distinctClozeGroups } from "./card-editor/cloze";
import { TemplateEntryForm } from "./card-editor/TemplateEntryForm";

interface CardEditorScreenProps {
  deck: Deck;
  decks?: Deck[];
  card: Flashcard | OrganizedCard | null;
  onCancel: () => void;
  onSaved: (card: OrganizedCard, continueAdding: boolean) => void | Promise<void>;
  onTemplateGenerated: (
    result: GenerateTemplateCardsResult,
    continueAdding: boolean,
    template: CardTemplate,
  ) => void | Promise<void>;
  onDirtyChange: (dirty: boolean) => void;
  onMoveCopy?: () => void;
}

export function CardEditorScreen({
  deck,
  decks: providedDecks,
  card,
  onCancel,
  onSaved,
  onTemplateGenerated,
  onDirtyChange,
  onMoveCopy,
}: CardEditorScreenProps) {
  const existing = card !== null && card.noteId !== "";
  const [loadedDecks, setLoadedDecks] = useState<Deck[]>(providedDecks ?? []);
  const availableDecks = useMemo(() => {
    const byId = new Map<string, Deck>();
    for (const candidate of [deck, ...loadedDecks]) byId.set(candidate.id, candidate);
    return [...byId.values()];
  }, [deck, loadedDecks]);
  const [selectedDeckId, setSelectedDeckId] = useState(card?.deckId ?? deck.id);
  const selectedDeck =
    availableDecks.find((candidate) => candidate.id === selectedDeckId) ?? deck;

  const [templates, setTemplates] = useState<CardTemplate[]>([]);
  const [selectedTemplateKey, setSelectedTemplateKey] = useState("basic");
  const selectedCustomTemplate = templates.find(
    (template) => template.id === selectedTemplateKey,
  );
  const builtInCardType: BuiltInCardType =
    selectedTemplateKey === "cloze" ? "cloze" : "basic";
  const [isLoadingType, setIsLoadingType] = useState(existing);
  const [reverseEnabled, setReverseEnabled] = useState(false);
  const [clozeSource, setClozeSource] = useState("");
  const [clozeMode, setClozeMode] = useState<ClozeMode>("one_at_a_time");

  const [frontDoc, setFrontDoc] = useState<RichDocument>(() =>
    card ? cloneDocument(card.frontDoc) : cloneDocument(EMPTY_RICH_DOCUMENT),
  );
  const [backDoc, setBackDoc] = useState<RichDocument>(() =>
    card ? cloneDocument(card.backDoc) : cloneDocument(EMPTY_RICH_DOCUMENT),
  );
  const [frontText, setFrontText] = useState(card?.frontText ?? "");
  const [backText, setBackText] = useState(card?.backText ?? "");
  const [assets, setAssets] = useState<Asset[]>(card?.assets ?? []);
  const [tags, setTags] = useState<string[]>(
    card && "tags" in card ? card.tags : [],
  );
  const [tagSuggestions, setTagSuggestions] = useState<TagSummary[]>([]);
  const [templateValues, setTemplateValues] = useState<TemplateValueInput[]>([]);
  const [templatePreviews, setTemplatePreviews] = useState<TemplateCardPreview[]>([]);
  const [isPreviewingTemplate, setIsPreviewingTemplate] = useState(false);

  const [isSaving, setIsSaving] = useState(false);
  const savingRef = useRef(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [duplicateMatches, setDuplicateMatches] = useState<DuplicateCard[]>([]);
  const [moveCopyOpen, setMoveCopyOpen] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [previewAnswer, setPreviewAnswer] = useState(false);
  const [editorGeneration, setEditorGeneration] = useState(0);
  const [dirty, setDirty] = useState(false);

  const title = existing ? "Edit flashcard" : "Add flashcard";
  const hasFront = !isDocumentEmpty(frontText, frontDoc);
  const hasBack = !isDocumentEmpty(backText, backDoc);
  const clozeGroups = useMemo(() => distinctClozeGroups(clozeSource), [clozeSource]);
  const templateValuesComplete =
    selectedCustomTemplate !== undefined &&
    templateValues.length === selectedCustomTemplate.fields.length &&
    templateValues.every((entry) => entry.value.trim() !== "");
  const contentReady = selectedCustomTemplate
    ? templateValuesComplete
    : builtInCardType === "cloze"
      ? clozeSource.trim() !== "" && clozeGroups.length > 0
      : hasFront && hasBack;
  const canSave =
    contentReady && selectedDeckId !== "" && !isSaving && !isLoadingType;
  const assetIds = useMemo(() => new Set(assets.map((asset) => asset.id)), [assets]);

  useEffect(() => {
    onDirtyChange(dirty);
    return () => onDirtyChange(false);
  }, [dirty, onDirtyChange]);

  useEffect(() => {
    if (providedDecks) {
      setLoadedDecks(providedDecks);
      return;
    }
    let current = true;
    void listDecks()
      .then((decks) => {
        if (current) setLoadedDecks(decks);
      })
      .catch((error: unknown) => {
        if (current) {
          setErrorMessage(
            `The deck list could not be loaded: ${getErrorMessage(error)}`,
          );
        }
      });
    return () => {
      current = false;
    };
  }, [providedDecks]);

  useEffect(() => {
    let current = true;
    void listCardTemplates()
      .then((loaded) => {
        if (current) setTemplates(loaded);
      })
      .catch((error: unknown) => {
        if (current) {
          setErrorMessage(
            `Card templates could not be loaded: ${getErrorMessage(error)}`,
          );
        }
      });
    return () => {
      current = false;
    };
  }, []);

  useEffect(() => {
    void listTags()
      .then(setTagSuggestions)
      .catch((error: unknown) => {
        console.error("Unable to load tag suggestions:", getErrorMessage(error));
      });
  }, []);

  useEffect(() => {
    if (!existing || !card) {
      setIsLoadingType(false);
      return;
    }
    let current = true;
    setIsLoadingType(true);
    void getCardTypeInfo(card.noteId)
      .then((info) => {
        if (!current) return;
        setSelectedTemplateKey(info.cardType);
        setReverseEnabled(info.reverseEnabled);
        setClozeSource(info.clozeSource);
        setClozeMode(info.clozeMode);
      })
      .catch((error: unknown) => {
        if (current) setErrorMessage(getErrorMessage(error));
      })
      .finally(() => {
        if (current) setIsLoadingType(false);
      });
    return () => {
      current = false;
    };
  }, [card, existing]);

  useEffect(() => {
    if (!existing || !card || "tags" in card) return;
    let current = true;
    void getOrganizedCard(card.noteId)
      .then((organized) => {
        if (current) setTags(organized.tags);
      })
      .catch((error: unknown) => {
        if (current) setErrorMessage(getErrorMessage(error));
      });
    return () => {
      current = false;
    };
  }, [card, existing]);

  useEffect(() => {
    if (!selectedCustomTemplate) {
      setTemplateValues([]);
      setTemplatePreviews([]);
      return;
    }
    setTemplateValues((current) =>
      selectedCustomTemplate.fields.map((field) => ({
        fieldId: field.id,
        value: current.find((entry) => entry.fieldId === field.id)?.value ?? "",
      })),
    );
    setTemplatePreviews([]);
  }, [selectedCustomTemplate]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent): void {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        if (canSave) void save(false, false);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  function addAsset(asset: Asset): void {
    if (!assetIds.has(asset.id)) setAssets((current) => [...current, asset]);
    markDirty();
  }

  function markDirty(): void {
    if (savingRef.current) return;
    setDirty(true);
    setErrorMessage(null);
    setDuplicateMatches([]);
    setTemplatePreviews([]);
  }

  async function buildTemplatePreview(): Promise<void> {
    if (!selectedCustomTemplate || !templateValuesComplete || isPreviewingTemplate) return;
    setIsPreviewingTemplate(true);
    setErrorMessage(null);
    try {
      setTemplatePreviews(
        await previewTemplateCards({
          templateId: selectedCustomTemplate.id,
          values: templateValues,
        }),
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsPreviewingTemplate(false);
    }
  }

  async function save(
    continueAdding: boolean,
    allowDuplicate: boolean,
  ): Promise<void> {
    if (!contentReady) {
      setErrorMessage(
        selectedCustomTemplate
          ? "Fill in every template field before generating cards."
          : builtInCardType === "cloze"
            ? "Write a statement and mark at least one blank."
            : "Add content to both the question and answer sides.",
      );
      return;
    }
    if (savingRef.current) return;

    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);

    try {
      if (selectedCustomTemplate) {
        const result = await generateCardsFromTemplate({
          deckId: selectedDeckId,
          templateId: selectedCustomTemplate.id,
          values: templateValues,
          tags,
        });
        setDirty(false);
        setTemplatePreviews([]);
        await onTemplateGenerated(result, continueAdding, selectedCustomTemplate);
        if (continueAdding) {
          setTemplateValues(
            selectedCustomTemplate.fields.map((field) => ({
              fieldId: field.id,
              value: "",
            })),
          );
          setTags([]);
        }
        return;
      }

      if (
        builtInCardType === "basic" &&
        !allowDuplicate &&
        frontText.trim() !== "" &&
        backText.trim() !== ""
      ) {
        const duplicates = await findDuplicateCards({
          deckId: selectedDeckId,
          frontText,
          backText,
          excludeNoteId: existing && card ? card.noteId : undefined,
        });
        if (duplicates.length > 0) {
          setDuplicateMatches(duplicates);
          return;
        }
      }

      const common = {
        deckId: selectedDeckId,
        cardType: builtInCardType,
        reverseEnabled: builtInCardType === "basic" && reverseEnabled,
        clozeSource: builtInCardType === "cloze" ? clozeSource : "",
        clozeMode,
        frontDoc,
        backDoc,
        frontText,
        backText,
        tags,
        allowDuplicate,
      };
      const saved =
        existing && card
          ? await updateGeneratedCard({
              ...common,
              noteId: card.noteId,
              revision: card.revision,
            })
          : await createGeneratedCard(common);

      setDirty(false);
      setDuplicateMatches([]);
      try {
        await onSaved(saved, continueAdding && !existing);
        if (continueAdding && !existing) resetForAnother();
      } catch (error: unknown) {
        setErrorMessage(
          `The flashcard was saved, but the screen could not finish updating: ${getErrorMessage(error)}`,
        );
      }
    } catch (error: unknown) {
      if (getErrorCode(error) === "duplicate_card") {
        setDuplicateMatches([
          {
            noteId: "",
            deckId: selectedDeckId,
            frontText: builtInCardType === "cloze" ? clozeSource : frontText,
            backText: builtInCardType === "cloze" ? "Fill in the blank" : backText,
          },
        ]);
      } else {
        setErrorMessage(getErrorMessage(error));
      }
    } finally {
      savingRef.current = false;
      setIsSaving(false);
    }
  }

  function resetForAnother(): void {
    setFrontDoc(cloneDocument(EMPTY_RICH_DOCUMENT));
    setBackDoc(cloneDocument(EMPTY_RICH_DOCUMENT));
    setFrontText("");
    setBackText("");
    setAssets([]);
    setTags([]);
    setReverseEnabled(false);
    setClozeSource("");
    setClozeMode("one_at_a_time");
    setPreviewOpen(false);
    setPreviewAnswer(false);
    setEditorGeneration((value) => value + 1);
  }

  function openMoveCopy(): void {
    if (!existing || !card || dirty) return;
    if (onMoveCopy) {
      onMoveCopy();
      return;
    }
    setMoveCopyOpen(true);
  }

  function cancel(): void {
    if (isSaving) return;
    if (dirty && !window.confirm("Discard the changes to this flashcard?")) return;
    onCancel();
  }

  const saveLabel = selectedCustomTemplate
    ? `Generate ${selectedCustomTemplate.rules.length} ${
        selectedCustomTemplate.rules.length === 1 ? "card" : "cards"
      }`
    : "Save card";

  return (
    <div className="card-editor-screen" aria-busy={isSaving || isLoadingType}>
      <header className="screen-header card-editor-header">
        <div>
          <button type="button" className="back-button" disabled={isSaving} onClick={cancel}>
            ← {deck.name}
          </button>
          <h1>{title}</h1>
        </div>
        <div className="header-actions">
          {existing ? (
            <button
              type="button"
              className="secondary-button"
              disabled={isSaving || dirty}
              title={dirty ? "Save or discard edits before moving this card" : undefined}
              onClick={openMoveCopy}
            >
              Move / copy
            </button>
          ) : null}
          {!selectedCustomTemplate && builtInCardType === "basic" ? (
            <button
              type="button"
              className="secondary-button"
              disabled={isSaving}
              onClick={() => {
                setPreviewOpen((value) => !value);
                setPreviewAnswer(false);
              }}
            >
              {previewOpen ? "Hide preview" : "Preview card"}
            </button>
          ) : null}
          <button type="button" className="secondary-button" disabled={isSaving} onClick={cancel}>
            Cancel
          </button>
          {!existing ? (
            <button
              type="button"
              className="secondary-button"
              disabled={!canSave}
              onClick={() => void save(true, false)}
            >
              {selectedCustomTemplate ? "Generate + another" : "Save + another"}
            </button>
          ) : null}
          <button
            type="button"
            className="primary-button"
            disabled={!canSave}
            onClick={() => void save(false, false)}
          >
            {isSaving ? (selectedCustomTemplate ? "Generating…" : "Saving…") : saveLabel}
          </button>
        </div>
      </header>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}

      {duplicateMatches.length > 0 ? (
        <section className="duplicate-warning" aria-labelledby="duplicate-warning-title">
          <div>
            <h2 id="duplicate-warning-title">This looks like an exact duplicate</h2>
            <p>
              A card in {selectedDeck.name} already has the same normalized question and
              answer. Review it before creating another copy.
            </p>
            {duplicateMatches
              .filter((match) => match.noteId !== "")
              .slice(0, 3)
              .map((match) => (
                <article key={match.noteId}>
                  <strong>{summarize(match.frontText)}</strong>
                  <span>{summarize(match.backText)}</span>
                </article>
              ))}
          </div>
          <div className="duplicate-warning-actions">
            <button
              type="button"
              className="secondary-button"
              disabled={isSaving}
              onClick={() => setDuplicateMatches([])}
            >
              Keep editing
            </button>
            <button
              type="button"
              className="danger-button"
              disabled={isSaving}
              onClick={() => void save(false, true)}
            >
              Save duplicate anyway
            </button>
          </div>
        </section>
      ) : null}

      <div className={previewOpen ? "card-editor-layout with-preview" : "card-editor-layout"}>
        <div className="card-editor-fields">
          <label className="card-template-control">
            <span>Card template</span>
            <select
              aria-label="Card template"
              value={selectedTemplateKey}
              disabled={isSaving || existing || isLoadingType}
              onChange={(event) => {
                setSelectedTemplateKey(event.currentTarget.value);
                setPreviewOpen(false);
                setPreviewAnswer(false);
                markDirty();
              }}
            >
              <option value="basic">Basic</option>
              <option value="cloze">Fill in the blank</option>
              {templates.length > 0 ? (
                <optgroup label="Reusable templates">
                  {templates.map((template) => (
                    <option key={template.id} value={template.id}>
                      {template.name}
                    </option>
                  ))}
                </optgroup>
              ) : null}
            </select>
            <small>
              {existing
                ? "The built-in card type is locked after saving so schedules remain attached to the correct presentations."
                : "Choose a built-in card type or a reusable bulk-entry template."}
            </small>
          </label>

          <label className="card-deck-control">
            <span>Deck</span>
            <select
              aria-label="Deck"
              value={selectedDeckId}
              disabled={isSaving}
              onChange={(event) => {
                setSelectedDeckId(event.currentTarget.value);
                markDirty();
              }}
            >
              {availableDecks.map((candidate) => (
                <option key={candidate.id} value={candidate.id}>
                  {candidate.name}
                </option>
              ))}
            </select>
            <small>A card belongs to one deck and can have any number of tags.</small>
          </label>

          {selectedCustomTemplate ? (
            <TemplateEntryForm
              template={selectedCustomTemplate}
              values={templateValues}
              previews={templatePreviews}
              disabled={isSaving}
              isPreviewing={isPreviewingTemplate}
              onValuesChange={(values) => {
                setTemplateValues(values);
                markDirty();
              }}
              onPreview={() => void buildTemplatePreview()}
            />
          ) : builtInCardType === "cloze" ? (
            <ClozeEditor
              source={clozeSource}
              mode={clozeMode}
              disabled={isSaving}
              onSourceChange={(source) => {
                setClozeSource(source);
                markDirty();
              }}
              onModeChange={(mode) => {
                setClozeMode(mode);
                markDirty();
              }}
            />
          ) : (
            <>
              <RichTextEditor
                key={`front-${editorGeneration}`}
                label="Question"
                document={frontDoc}
                assets={assets}
                disabled={isSaving}
                onAssetImported={addAsset}
                onChange={(document: RichDocument, plainText: string) => {
                  setFrontDoc(document);
                  setFrontText(plainText);
                  markDirty();
                }}
              />
              <RichTextEditor
                key={`back-${editorGeneration}`}
                label="Answer"
                document={backDoc}
                assets={assets}
                disabled={isSaving}
                onAssetImported={addAsset}
                onChange={(document: RichDocument, plainText: string) => {
                  setBackDoc(document);
                  setBackText(plainText);
                  markDirty();
                }}
                minHeight={220}
              />
              <label className="switch-row reverse-card-option">
                <span>
                  <strong>Create reverse card</strong>
                  <small>
                    Study both directions with separate schedules and review histories.
                  </small>
                </span>
                <input
                  type="checkbox"
                  checked={reverseEnabled}
                  disabled={isSaving}
                  onChange={(event) => {
                    setReverseEnabled(event.currentTarget.checked);
                    markDirty();
                  }}
                />
              </label>
            </>
          )}

          <TagInput
            tags={tags}
            suggestions={tagSuggestions}
            disabled={isSaving}
            onChange={(nextTags) => {
              setTags(nextTags);
              markDirty();
            }}
          />
        </div>

        {previewOpen && builtInCardType === "basic" && !selectedCustomTemplate ? (
          <aside className="editor-preview-panel">
            <div className="preview-panel-heading">
              <div>
                <span className="eyebrow">LIVE PREVIEW</span>
                <h2>{previewAnswer ? "Answer" : "Question"}</h2>
              </div>
              <button
                type="button"
                className="small-primary-button"
                disabled={isSaving}
                onClick={() => setPreviewAnswer((value) => !value)}
              >
                {previewAnswer ? "Show question" : "Reveal answer"}
              </button>
            </div>
            {tags.length > 0 ? (
              <div className="preview-tag-list">
                {tags.map((tag) => (
                  <span className="tag-chip static" key={tag}>
                    {tag}
                  </span>
                ))}
              </div>
            ) : null}
            <div className="flashcard-preview-surface">
              <RichTextView
                document={previewAnswer ? backDoc : frontDoc}
                assets={assets}
                onLinkError={setErrorMessage}
              />
            </div>
          </aside>
        ) : null}
      </div>

      {moveCopyOpen && card ? (
        <MoveCopyModal
          cards={[{ noteId: card.noteId, deckId: card.deckId }]}
          decks={availableDecks}
          onClose={() => setMoveCopyOpen(false)}
          onComplete={async () => {
            setMoveCopyOpen(false);
            const refreshed = await getOrganizedCard(card.noteId);
            await onSaved(refreshed, false);
          }}
        />
      ) : null}
    </div>
  );
}

function summarize(value: string): string {
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (normalized === "") return "Image card";
  return normalized.length > 120 ? `${normalized.slice(0, 117)}…` : normalized;
}

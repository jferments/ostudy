import { useEffect, useMemo, useRef, useState } from "react";

import { RichTextEditor } from "../editor/RichTextEditor";
import { RichTextView } from "../editor/RichTextView";
import {
  EMPTY_RICH_DOCUMENT,
  cloneDocument,
  isDocumentEmpty,
} from "../editor/richText";
import {
  checkCardDuplicates,
  createOrganizedCard,
  getErrorMessage,
  updateOrganizedCard,
  type Asset,
  type Deck,
  type DuplicateMatch,
  type Flashcard,
  type RichDocument,
  type Tag,
  type TagSummary,
} from "../ipc";

interface CardEditorScreenProps {
  deck: Deck;
  card: Flashcard | null;
  initialTags: Tag[];
  availableTags: TagSummary[];
  onCancel: () => void;
  onSaved: (card: Flashcard, continueAdding: boolean) => void | Promise<void>;
  onDirtyChange: (dirty: boolean) => void;
}

export function CardEditorScreen({
  deck,
  card,
  initialTags,
  availableTags,
  onCancel,
  onSaved,
  onDirtyChange,
}: CardEditorScreenProps) {
  const [frontDoc, setFrontDoc] = useState<RichDocument>(() =>
    card ? cloneDocument(card.frontDoc) : cloneDocument(EMPTY_RICH_DOCUMENT),
  );
  const [backDoc, setBackDoc] = useState<RichDocument>(() =>
    card ? cloneDocument(card.backDoc) : cloneDocument(EMPTY_RICH_DOCUMENT),
  );
  const [frontText, setFrontText] = useState(card?.frontText ?? "");
  const [backText, setBackText] = useState(card?.backText ?? "");
  const [assets, setAssets] = useState<Asset[]>(card?.assets ?? []);
  const [tagNames, setTagNames] = useState(() => initialTags.map((tag) => tag.name));
  const [tagDraft, setTagDraft] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const savingRef = useRef(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [previewAnswer, setPreviewAnswer] = useState(false);
  const [editorGeneration, setEditorGeneration] = useState(0);
  const [dirty, setDirty] = useState(false);
  const [duplicateMatches, setDuplicateMatches] = useState<DuplicateMatch[]>([]);
  const [pendingContinueAdding, setPendingContinueAdding] = useState(false);

  const title = card ? "Edit flashcard" : "Add flashcard";
  const hasFront = !isDocumentEmpty(frontText, frontDoc);
  const hasBack = !isDocumentEmpty(backText, backDoc);
  const canSave = hasFront && hasBack && !isSaving;

  useEffect(() => {
    onDirtyChange(dirty);
    return () => onDirtyChange(false);
  }, [dirty, onDirtyChange]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        if (canSave) void save(false, false);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  const assetIds = useMemo(() => new Set(assets.map((asset) => asset.id)), [assets]);
  const selectedTagKeys = useMemo(
    () => new Set(tagNames.map((tag) => tag.toLocaleLowerCase())),
    [tagNames],
  );
  const tagSuggestions = useMemo(
    () =>
      availableTags.filter(
        (tag) => !selectedTagKeys.has(tag.name.toLocaleLowerCase()),
      ),
    [availableTags, selectedTagKeys],
  );

  function addAsset(asset: Asset): void {
    if (!assetIds.has(asset.id)) {
      setAssets((current) => [...current, asset]);
    }
    markDirty();
  }

  function markDirty(): void {
    if (savingRef.current) return;
    setDirty(true);
    setErrorMessage(null);
    setDuplicateMatches([]);
  }

  function commitTagDraft(): void {
    const next = mergeTags(tagNames, tagDraft.split(","));
    setTagNames(next);
    setTagDraft("");
  }

  function addSuggestedTag(name: string): void {
    setTagNames((current) => mergeTags(current, [name]));
    setTagDraft("");
    markDirty();
  }

  function removeTag(name: string): void {
    setTagNames((current) => current.filter((tag) => tag !== name));
    markDirty();
  }

  async function save(
    continueAdding: boolean,
    bypassDuplicateWarning: boolean,
  ): Promise<void> {
    if (!hasFront || !hasBack) {
      setErrorMessage("Add content to both the question and answer sides.");
      return;
    }
    if (savingRef.current) return;

    const tags = mergeTags(tagNames, tagDraft.split(","));
    setTagNames(tags);
    setTagDraft("");
    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);

    if (!bypassDuplicateWarning) {
      try {
        const matches = await checkCardDuplicates(
          frontText,
          backText,
          card?.noteId,
        );
        if (matches.length > 0) {
          setDuplicateMatches(matches);
          setPendingContinueAdding(continueAdding);
          savingRef.current = false;
          setIsSaving(false);
          return;
        }
      } catch (error: unknown) {
        setErrorMessage(getErrorMessage(error));
        savingRef.current = false;
        setIsSaving(false);
        return;
      }
    }

    let saved: Flashcard;
    try {
      saved = card
        ? await updateOrganizedCard({
            noteId: card.noteId,
            revision: card.revision,
            frontDoc,
            backDoc,
            frontText,
            backText,
            tags,
          })
        : await createOrganizedCard({
            deckId: deck.id,
            frontDoc,
            backDoc,
            frontText,
            backText,
            tags,
          });
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
      savingRef.current = false;
      setIsSaving(false);
      return;
    }

    // The database write has succeeded. Never describe a later refresh problem
    // as a failed save, because retrying would create a duplicate card.
    setDirty(false);
    setDuplicateMatches([]);
    try {
      await onSaved(saved, continueAdding && !card);
      if (continueAdding && !card) {
        setFrontDoc(cloneDocument(EMPTY_RICH_DOCUMENT));
        setBackDoc(cloneDocument(EMPTY_RICH_DOCUMENT));
        setFrontText("");
        setBackText("");
        setAssets([]);
        setTagNames([]);
        setTagDraft("");
        setPreviewOpen(false);
        setPreviewAnswer(false);
        setEditorGeneration((value) => value + 1);
      }
    } catch (error: unknown) {
      setErrorMessage(
        `The flashcard was saved, but the screen could not finish updating: ${getErrorMessage(error)}`,
      );
    } finally {
      savingRef.current = false;
      setIsSaving(false);
    }
  }

  function cancel(): void {
    if (isSaving) return;
    if (dirty && !window.confirm("Discard the changes to this flashcard?")) return;
    onCancel();
  }

  return (
    <div className="card-editor-screen" aria-busy={isSaving}>
      <header className="screen-header card-editor-header">
        <div>
          <button type="button" className="back-button" disabled={isSaving} onClick={cancel}>
            ← {deck.name}
          </button>
          <h1>{title}</h1>
        </div>
        <div className="header-actions">
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
          <button type="button" className="secondary-button" disabled={isSaving} onClick={cancel}>
            Cancel
          </button>
          {!card ? (
            <button
              type="button"
              className="secondary-button"
              disabled={!canSave}
              onClick={() => void save(true, false)}
            >
              Save + another
            </button>
          ) : null}
          <button
            type="button"
            className="primary-button"
            disabled={!canSave}
            onClick={() => void save(false, false)}
          >
            {isSaving ? "Saving…" : "Save card"}
          </button>
        </div>
      </header>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
      {duplicateMatches.length > 0 ? (
        <section className="duplicate-warning" role="alert">
          <div>
            <strong>An identical question and answer already exist.</strong>
            <p>
              {duplicateMatches.slice(0, 4).map((match) => match.deckName).join(", ")}
              {duplicateMatches.length > 4
                ? ` and ${duplicateMatches.length - 4} more`
                : ""}
            </p>
          </div>
          <div className="duplicate-warning-actions">
            <button
              type="button"
              className="secondary-button"
              onClick={() => setDuplicateMatches([])}
            >
              Keep editing
            </button>
            <button
              type="button"
              className="primary-button"
              onClick={() => void save(pendingContinueAdding, true)}
            >
              Save anyway
            </button>
          </div>
        </section>
      ) : null}

      <div className={previewOpen ? "card-editor-layout with-preview" : "card-editor-layout"}>
        <div className="card-editor-fields">
          <RichTextEditor
            key={`front-${editorGeneration}`}
            label="Question"
            document={frontDoc}
            assets={assets}
            disabled={isSaving}
            onAssetImported={addAsset}
            onChange={(document, plainText) => {
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
            onChange={(document, plainText) => {
              setBackDoc(document);
              setBackText(plainText);
              markDirty();
            }}
            minHeight={220}
          />

          <section className="card-tag-editor" aria-labelledby="card-tags-heading">
            <div className="card-tag-heading">
              <div>
                <span className="eyebrow">ORGANIZE</span>
                <h2 id="card-tags-heading">Tags</h2>
              </div>
              <span>{tagNames.length}/50</span>
            </div>
            {tagNames.length > 0 ? (
              <div className="tag-chip-list" aria-label="Selected tags">
                {tagNames.map((tag) => (
                  <button
                    key={tag}
                    type="button"
                    className="tag-chip removable-tag"
                    disabled={isSaving}
                    onClick={() => removeTag(tag)}
                    title={`Remove ${tag}`}
                  >
                    {tag} <span aria-hidden="true">×</span>
                  </button>
                ))}
              </div>
            ) : null}
            <label className="tag-input-field">
              <span>Add tags</span>
              <input
                list="card-tag-suggestions"
                value={tagDraft}
                disabled={isSaving || tagNames.length >= 50}
                maxLength={500}
                placeholder="cardiology, high yield"
                onChange={(event) => {
                  setTagDraft(event.currentTarget.value);
                  markDirty();
                }}
                onBlur={() => {
                  if (tagDraft.trim() !== "") commitTagDraft();
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === ",") {
                    event.preventDefault();
                    if (tagDraft.trim() !== "") commitTagDraft();
                  }
                }}
              />
              <small>Press Enter or type commas to add several tags.</small>
            </label>
            <datalist id="card-tag-suggestions">
              {tagSuggestions.map((tag) => (
                <option key={tag.id} value={tag.name} />
              ))}
            </datalist>
            {tagSuggestions.length > 0 ? (
              <div className="tag-suggestion-row" aria-label="Existing tags">
                {tagSuggestions.slice(0, 8).map((tag) => (
                  <button
                    key={tag.id}
                    type="button"
                    className="tag-suggestion"
                    disabled={isSaving || tagNames.length >= 50}
                    onClick={() => addSuggestedTag(tag.name)}
                  >
                    + {tag.name}
                  </button>
                ))}
              </div>
            ) : null}
          </section>
        </div>

        {previewOpen ? (
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
    </div>
  );
}

function mergeTags(existing: string[], additions: string[]): string[] {
  const result: string[] = [];
  const keys = new Set<string>();
  for (const rawTag of [...existing, ...additions]) {
    const tag = rawTag.trim().replace(/\s+/gu, " ");
    if (tag === "") continue;
    const key = tag.toLocaleLowerCase();
    if (!keys.has(key)) {
      keys.add(key);
      result.push(tag);
    }
  }
  return result.slice(0, 50);
}

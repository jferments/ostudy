import { useEffect, useMemo, useRef, useState } from "react";

import { RichTextEditor } from "../editor/RichTextEditor";
import { RichTextView } from "../editor/RichTextView";
import {
  EMPTY_RICH_DOCUMENT,
  cloneDocument,
  isDocumentEmpty,
} from "../editor/richText";
import {
  createCard,
  getErrorMessage,
  updateCard,
  type Asset,
  type Deck,
  type Flashcard,
  type RichDocument,
} from "../ipc";

interface CardEditorScreenProps {
  deck: Deck;
  card: Flashcard | null;
  onCancel: () => void;
  onSaved: (card: Flashcard, continueAdding: boolean) => void | Promise<void>;
  onDirtyChange: (dirty: boolean) => void;
}

export function CardEditorScreen({
  deck,
  card,
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
  const [isSaving, setIsSaving] = useState(false);
  const savingRef = useRef(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [previewAnswer, setPreviewAnswer] = useState(false);
  const [editorGeneration, setEditorGeneration] = useState(0);
  const [dirty, setDirty] = useState(false);

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
        if (canSave) void save(false);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  const assetIds = useMemo(() => new Set(assets.map((asset) => asset.id)), [assets]);

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
  }

  async function save(continueAdding: boolean): Promise<void> {
    if (!hasFront || !hasBack) {
      setErrorMessage("Add content to both the question and answer sides.");
      return;
    }
    if (savingRef.current) return;

    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);

    let saved: Flashcard;
    try {
      saved = card
        ? await updateCard({
            noteId: card.noteId,
            revision: card.revision,
            frontDoc,
            backDoc,
            frontText,
            backText,
          })
        : await createCard({
            deckId: deck.id,
            frontDoc,
            backDoc,
            frontText,
            backText,
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
    try {
      await onSaved(saved, continueAdding && !card);
      if (continueAdding && !card) {
        setFrontDoc(cloneDocument(EMPTY_RICH_DOCUMENT));
        setBackDoc(cloneDocument(EMPTY_RICH_DOCUMENT));
        setFrontText("");
        setBackText("");
        setAssets([]);
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
              onClick={() => void save(true)}
            >
              Save + another
            </button>
          ) : null}
          <button
            type="button"
            className="primary-button"
            disabled={!canSave}
            onClick={() => void save(false)}
          >
            {isSaving ? "Saving…" : "Save card"}
          </button>
        </div>
      </header>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}

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

import { useEffect, useMemo, useState } from "react";

import type { ShowToast } from "../../../components/Toast";
import { RICH_DOCUMENT_SCHEMA_VERSION } from "../../../editor/adapter";
import { EMPTY_RICH_DOCUMENT, cloneDocument } from "../../../editor/richText";
import { linkCardNoteToAnnotation } from "../../../ipc/annotationCards";
import type { CardNote } from "../../../ipc/cards";
import { getErrorMessage, type RichDocument } from "../../../ipc/common";
import { listDecks, type DeckSummary } from "../../../ipc/decks";
import { Button } from "../../../ui/primitives/Button";
import { LoadingState } from "../../../ui/primitives/EmptyState";
import { InlineNotice } from "../../../ui/primitives/InlineNotice";
import { CardEditorScreen } from "../../CardEditorScreen";

/** The mark a new card is made from, with the words it holds. */
export interface CardMark {
  id: string;
  quote: string;
}

interface MarkCardEditorProps {
  mark: CardMark;
  /** The deck a card from this document goes to unless the learner picks another. */
  suggestedDeckId: string | null;
  onClose: () => void;
  /** After a save; `closed` is false when the learner chose to add another. */
  onSaved: (closed: boolean) => void;
  showToast: ShowToast;
}

type DeckLoad =
  | { state: "loading" }
  | { state: "failed"; message: string }
  | { state: "ready"; decks: DeckSummary[] };

/**
 * A new-card editor beside the pages, its answer filled with the marked words.
 * Each card saved here is linked to the mark, which also cites its page.
 */
export function MarkCardEditor({ mark, suggestedDeckId, onClose, onSaved, showToast }: MarkCardEditorProps) {
  const [load, setLoad] = useState<DeckLoad>({ state: "loading" });
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let active = true;
    setLoad({ state: "loading" });
    listDecks()
      .then((decks) => {
        if (active) setLoad({ state: "ready", decks: decks.filter((deck) => deck.archivedAt === null) });
      })
      .catch((error: unknown) => {
        if (active) setLoad({ state: "failed", message: getErrorMessage(error) });
      });
    return () => { active = false; };
  }, [attempt]);

  const decks = load.state === "ready" ? load.decks : [];
  const deck = decks.find((candidate) => candidate.id === suggestedDeckId) ?? decks[0] ?? null;
  const draft = useMemo(() => deck ? newCardFromQuote(deck.id, mark.quote) : null, [deck, mark.quote]);

  async function link(noteIds: readonly string[], closed: boolean) {
    try {
      for (const noteId of noteIds) await linkCardNoteToAnnotation(noteId, mark.id);
      showToast("success", noteIds.length === 1 ? "Card saved." : `${noteIds.length} cards saved.`);
    } catch (error: unknown) {
      showToast("error", `The card was saved, but it could not be linked to the mark: ${getErrorMessage(error)}`);
    }
    onSaved(closed);
  }

  if (load.state === "loading") return <LoadingState label="Opening the card editor…" />;
  if (load.state === "failed") {
    return (
      <div className="tw:grid tw:justify-items-start tw:gap-3 tw:p-4">
        <InlineNotice kind="error">{load.message}</InlineNotice>
        <div className="tw:flex tw:gap-2">
          <Button onClick={() => setAttempt((count) => count + 1)}>Try again</Button>
          <Button variant="text" onClick={onClose}>Close</Button>
        </div>
      </div>
    );
  }
  if (!deck || !draft) {
    return (
      <div className="tw:grid tw:justify-items-start tw:gap-3 tw:p-4">
        <InlineNotice kind="info">Cards go in a deck. Make a deck in the Library, then make cards from your marks.</InlineNotice>
        <Button variant="text" onClick={onClose}>Close</Button>
      </div>
    );
  }
  return (
    <CardEditorScreen
      key={`${mark.id}-${deck.id}`}
      deck={deck}
      decks={decks}
      card={draft}
      closeButton
      // An unsaved card stays in its draft, which the editor offers back next time.
      onDirtyChange={ignoreDirty}
      onCancel={onClose}
      onSaved={(saved, continueAdding) => link([saved.noteId], !continueAdding)}
      onQuestionSaved={(noteId, continueAdding) => link([noteId], !continueAdding)}
      onTemplateGenerated={(result, continueAdding) => link(result.createdNoteIds, !continueAdding)}
    />
  );
}

const ignoreDirty = () => undefined;

/** A basic card not yet saved, its answer the marked words, one paragraph a line. */
export function newCardFromQuote(deckId: string, quote: string): CardNote {
  const lines = quote.split(/\r?\n/);
  const backDoc: RichDocument = quote.trim() === ""
    ? cloneDocument(EMPTY_RICH_DOCUMENT)
    : {
      type: "doc",
      attrs: { schemaVersion: RICH_DOCUMENT_SCHEMA_VERSION },
      content: lines.map((line) => line === ""
        ? { type: "paragraph" }
        : { type: "paragraph", content: [{ type: "text", text: line }] }),
    };
  return {
    noteId: "",
    deckId,
    tags: [],
    generatedCardCount: 0,
    createdAt: 0,
    updatedAt: 0,
    revision: 0,
    cardType: "basic",
    frontDoc: cloneDocument(EMPTY_RICH_DOCUMENT),
    backDoc,
    frontText: "",
    backText: quote,
    reverseEnabled: false,
    assets: [],
  };
}

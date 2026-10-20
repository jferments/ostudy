import { useEffect, useMemo, useRef, useState } from "react";

import type { ShowToast } from "../../../components/Toast";
import { RICH_DOCUMENT_SCHEMA_VERSION } from "../../../editor/adapter";
import { EMPTY_RICH_DOCUMENT, cloneDocument } from "../../../editor/richText";
import { draftMarkQuestion, linkCardNoteToAnnotation } from "../../../ipc/annotationCards";
import type { CardNote } from "../../../ipc/cards";
import { createOperationId, getErrorMessage, type RichDocument } from "../../../ipc/common";
import { listDecks, type DeckSummary } from "../../../ipc/decks";
import { cancelLearningTask } from "../../../ipc/knowledge";
import { Button } from "../../../ui/primitives/Button";
import { Icon } from "../../../ui/icons/Icon";
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

/** The question the AI is writing for this mark, from the learner's request on. */
type Suggestion =
  | { state: "idle" }
  | { state: "writing"; operationId: string }
  | { state: "failed"; message: string }
  /** Written after the learner started editing, so it is offered rather than filled in. */
  | { state: "offered"; question: string };

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
  // A question the AI wrote fills in the card while the learner has not changed it yet.
  const [question, setQuestion] = useState("");
  const draft = useMemo(() => deck ? newCardFromQuote(deck.id, mark.quote, question) : null, [deck, mark.quote, question]);
  const dirty = useRef(false);
  const [suggestion, setSuggestion] = useState<Suggestion>({ state: "idle" });
  const writing = suggestion.state === "writing" ? suggestion.operationId : null;
  // Closing the editor or moving to another mark stops a question still being written.
  const current = useRef<string | null>(null);
  useEffect(() => () => {
    if (current.current) void cancelLearningTask(current.current).catch(() => undefined);
    current.current = null;
  }, [mark.id]);

  async function suggestQuestion() {
    const operationId = createOperationId();
    current.current = operationId;
    setSuggestion({ state: "writing", operationId });
    try {
      const drafted = await draftMarkQuestion(operationId, mark.id);
      if (current.current !== operationId) return;
      if (dirty.current) {
        setSuggestion({ state: "offered", question: drafted.question });
      } else {
        setQuestion(drafted.question);
        setSuggestion({ state: "idle" });
      }
    } catch (error: unknown) {
      if (current.current !== operationId) return;
      setSuggestion({ state: "failed", message: getErrorMessage(error) });
    } finally {
      if (current.current === operationId) current.current = null;
    }
  }

  function stopWriting() {
    if (!writing) return;
    current.current = null;
    setSuggestion({ state: "idle" });
    void cancelLearningTask(writing).catch(() => undefined);
  }

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
    <div>
      {mark.quote.trim() !== "" ? (
        <div className="tw:grid tw:gap-2 tw:pt-3" aria-live="polite">
          {writing ? (
            <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2 tw:text-small tw:text-muted-foreground">
              <span role="status">Writing a question…</span>
              <Button size="small" variant="text" onClick={stopWriting}>Stop</Button>
            </div>
          ) : (
            <div>
              <Button size="small" onClick={() => void suggestQuestion()}>
                <Icon name="sparkles" size={16} />Suggest a question
              </Button>
            </div>
          )}
          {suggestion.state === "failed" ? (
            <InlineNotice kind="warning">A question could not be written: {suggestion.message}</InlineNotice>
          ) : null}
          {suggestion.state === "offered" ? (
            <InlineNotice kind="info">
              Suggested question: <strong>{suggestion.question}</strong>
            </InlineNotice>
          ) : null}
        </div>
      ) : null}
      <CardEditorScreen
      key={`${mark.id}-${deck.id}-${question}`}
      deck={deck}
      decks={decks}
      card={draft}
      closeButton
      // An unsaved card stays in its draft, which the editor offers back next time.
      onDirtyChange={(changed) => { dirty.current = changed; }}
      onCancel={onClose}
      onSaved={(saved, continueAdding) => link([saved.noteId], !continueAdding)}
      onQuestionSaved={(noteId, continueAdding) => link([noteId], !continueAdding)}
      onTemplateGenerated={(result, continueAdding) => link(result.createdNoteIds, !continueAdding)}
      />
    </div>
  );
}

/**
 * A basic card not yet saved, its answer the marked words, one paragraph a
 * line, and its question empty unless one is given.
 */
export function newCardFromQuote(deckId: string, quote: string, question = ""): CardNote {
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
    frontDoc: question === ""
      ? cloneDocument(EMPTY_RICH_DOCUMENT)
      : {
        type: "doc",
        attrs: { schemaVersion: RICH_DOCUMENT_SCHEMA_VERSION },
        content: [{ type: "paragraph", content: [{ type: "text", text: question }] }],
      },
    backDoc,
    frontText: question,
    backText: quote,
    reverseEnabled: false,
    assets: [],
  };
}

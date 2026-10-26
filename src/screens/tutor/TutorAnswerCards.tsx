import { useEffect, useRef, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { bulkCreateCards } from "../../ipc/cards";
import { createOperationId, getErrorMessage } from "../../ipc/common";
import { listDecks, type DeckSummary } from "../../ipc/decks";
import { cancelLearningTask } from "../../ipc/knowledge";
import { draftTutorAnswerCards, type AnswerCardDraft } from "../../ipc/tutor";
import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { Checkbox, Textarea } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";

type Drafting =
  | { state: "writing" }
  | { state: "failed"; message: string }
  | { state: "ready"; cards: AnswerCardDraft[] };

/**
 * Cards the flashcard model drafts from one answer. The learner edits them,
 * leaves out the ones not wanted and adds the rest to a deck; a card the deck
 * already holds is skipped.
 */
export function TutorAnswerCards({ turnId, onClose }: { turnId: string; onClose: () => void }) {
  const [drafting, setDrafting] = useState<Drafting>({ state: "writing" });
  const [attempt, setAttempt] = useState(0);
  const [decks, setDecks] = useState<DeckSummary[] | null>(null);
  const [deckId, setDeckId] = useState("");
  const [unpicked, setUnpicked] = useState<ReadonlySet<number>>(new Set());
  const [adding, setAdding] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [added, setAdded] = useState<string | null>(null);
  const deckLane = useRequestLane();
  const writing = useRef<string | null>(null);

  useEffect(() => {
    const operationId = createOperationId();
    writing.current = operationId;
    setDrafting({ state: "writing" });
    draftTutorAnswerCards(operationId, turnId).then(
      (cards) => { if (writing.current === operationId) setDrafting({ state: "ready", cards }); },
      (failure: unknown) => {
        if (writing.current === operationId) setDrafting({ state: "failed", message: getErrorMessage(failure) });
      },
    ).finally(() => { if (writing.current === operationId) writing.current = null; });
    // Closing stops cards still being written.
    return () => {
      if (writing.current === operationId) void cancelLearningTask(operationId).catch(() => undefined);
      writing.current = null;
    };
  }, [turnId, attempt]);

  useEffect(() => {
    void latestResult(deckLane, listDecks).then(
      (value) => {
        if (value === undefined) return;
        const live = value.filter((deck) => deck.archivedAt === null);
        setDecks(live);
        setDeckId((current) => live.some((deck) => deck.id === current) ? current : live[0]?.id ?? "");
      },
      (failure: unknown) => { setDecks([]); setError(getErrorMessage(failure)); },
    );
  }, [deckLane]);

  const cards = drafting.state === "ready" ? drafting.cards : [];
  const picked = cards.flatMap((card, index) => unpicked.has(index) || !card.question.trim() || !card.answer.trim() ? [] : [card]);
  const deck = decks?.find((candidate) => candidate.id === deckId);

  function edit(index: number, change: Partial<AnswerCardDraft>): void {
    setDrafting((current) => current.state !== "ready" ? current : {
      state: "ready",
      cards: current.cards.map((card, at) => at === index ? { ...card, ...change } : card),
    });
  }

  async function add(): Promise<void> {
    if (!deck || picked.length === 0) return;
    setAdding(true);
    setError(null);
    try {
      const rows = picked.map((card) => ({ question: card.question.trim(), answer: card.answer.trim(), tags: [] }));
      const result = await bulkCreateCards({ deckId: deck.id, rows });
      const skipped = result.skippedDuplicates;
      setAdded(
        `${result.createdCount === 1 ? "1 card" : `${result.createdCount} cards`} added to ${deck.name}.`
        + (skipped > 0 ? ` ${skipped === 1 ? "1 was" : `${skipped} were`} already there.` : ""),
      );
    } catch (failure: unknown) {
      setError(getErrorMessage(failure));
    } finally {
      setAdding(false);
    }
  }

  const actions = added ? (
    <Button variant="primary" onClick={onClose}>Done</Button>
  ) : (
    <>
      <Button variant="secondary" onClick={onClose}>Cancel</Button>
      {drafting.state === "ready" && decks && decks.length > 0 ? (
        <Button variant="primary" disabled={adding || picked.length === 0} onClick={() => void add()}>
          {picked.length === 1 ? "Add 1 card" : `Add ${picked.length} cards`}
        </Button>
      ) : null}
    </>
  );

  return (
    <Sheet title="Make flashcards" onClose={onClose} actions={actions} busy={adding} width="medium">
      {added ? (
        <p role="status">{added}</p>
      ) : drafting.state === "writing" ? (
        <p role="status" className="tw:text-muted-foreground">Writing cards from this answer…</p>
      ) : drafting.state === "failed" ? (
        <div className="tw:grid tw:justify-items-start tw:gap-3">
          <InlineNotice kind="error">The cards could not be written: {drafting.message}</InlineNotice>
          <Button onClick={() => setAttempt((count) => count + 1)}>Try again</Button>
        </div>
      ) : (
        <div className="tw:grid tw:gap-4">
          <ul className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0" aria-label="Cards">
            {cards.map((card, index) => (
              <li key={index} className="tw:flex tw:items-start tw:gap-3">
                <Checkbox
                  className="tw:mt-2.5"
                  aria-label={`Keep card ${index + 1}`}
                  checked={!unpicked.has(index)}
                  disabled={adding}
                  onCheckedChange={(on) => setUnpicked((current) => {
                    const next = new Set(current);
                    if (on) next.delete(index);
                    else next.add(index);
                    return next;
                  })}
                />
                <div className="tw:grid tw:min-w-0 tw:flex-1 tw:gap-1">
                  <Textarea
                    aria-label={`Card ${index + 1} question`}
                    rows={2}
                    maxLength={300}
                    value={card.question}
                    disabled={adding || unpicked.has(index)}
                    onChange={(event) => edit(index, { question: event.target.value })}
                  />
                  <Textarea
                    aria-label={`Card ${index + 1} answer`}
                    rows={2}
                    maxLength={600}
                    className="tw:text-muted-foreground"
                    value={card.answer}
                    disabled={adding || unpicked.has(index)}
                    onChange={(event) => edit(index, { answer: event.target.value })}
                  />
                </div>
              </li>
            ))}
          </ul>
          {decks && decks.length === 0 ? (
            <InlineNotice>Cards go in a deck. Make a deck in the Library, then make cards from your answers.</InlineNotice>
          ) : decks && decks.length > 1 ? (
            <label className="tw:grid tw:min-w-0 tw:gap-1 tw:text-label tw:font-medium">
              <span>Deck</span>
              <NativeSelect value={deckId} disabled={adding} onChange={(event) => setDeckId(event.currentTarget.value)}>
                {decks.map((candidate) => <option key={candidate.id} value={candidate.id}>{candidate.name}</option>)}
              </NativeSelect>
            </label>
          ) : deck ? <p className="tw:text-small tw:text-muted-foreground">Cards go to {deck.name}.</p> : null}
        </div>
      )}
      {error ? <InlineNotice kind="error">The cards could not be added: {error}</InlineNotice> : null}
    </Sheet>
  );
}

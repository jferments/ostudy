import { useEffect, useMemo, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { analyzeBulkCards, bulkCreateCards } from "../../ipc/cards";
import { getErrorMessage } from "../../ipc/common";
import { listDecks, type DeckSummary } from "../../ipc/decks";
import type { LessonStep } from "../../ipc/lessons";
import { getSubjectLibrary } from "../../ipc/subjects";
import { Button } from "../../ui/primitives/Button";
import { Checkbox, Disclosure } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { slideCards } from "./slideCardRows";

/**
 * The live decks a lesson's cards can go to: its subject's decks in their
 * library order, or every live deck when the subject has none.
 */
async function lessonDecks(subjectId: string): Promise<DeckSummary[]> {
  const [decks, library] = await Promise.all([listDecks(), getSubjectLibrary()]);
  const live = decks.filter((deck) => deck.archivedAt === null);
  const byId = new Map(live.map((deck) => [deck.id, deck]));
  const own = library.decks
    .filter((placement) => placement.subjectId === subjectId)
    .sort((a, b) => a.sortOrder - b.sortOrder)
    .flatMap((placement) => byId.get(placement.deckId) ?? []);
  return own.length > 0 ? own : live;
}

/**
 * The cards a slide's step makes, to add to a deck in one go. Cards the deck
 * already holds are shown as in it and are not added again.
 */
export function SlideCards({ subjectId, step, showToast }: {
  subjectId: string;
  step: LessonStep;
  showToast: ShowToast;
}) {
  const cards = useMemo(() => slideCards(step), [step]);
  const [decks, setDecks] = useState<DeckSummary[] | null>(null);
  const [deckId, setDeckId] = useState("");
  const [inDeck, setInDeck] = useState<ReadonlySet<number> | null>(null);
  const [unpicked, setUnpicked] = useState<ReadonlySet<number>>(new Set());
  const [adding, setAdding] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const deckLane = useRequestLane();
  const duplicateLane = useRequestLane();

  useEffect(() => {
    void latestResult(deckLane, () => lessonDecks(subjectId)).then(
      (value) => {
        if (value === undefined) return;
        setDecks(value);
        setDeckId((current) => value.some((deck) => deck.id === current) ? current : value[0]?.id ?? "");
      },
      // Without decks there is nowhere to add cards, so the panel stays away.
      () => setDecks([]),
    );
  }, [deckLane, subjectId]);

  useEffect(() => {
    setUnpicked(new Set());
    setInDeck(null);
    if (deckId === "" || cards.length === 0) return;
    void latestResult(duplicateLane, () => analyzeBulkCards({ deckId, rows: cards })).then(
      (value) => {
        if (value !== undefined) setInDeck(new Set(value.duplicateRowIndices));
      },
      // Not knowing which are in the deck only means all are offered; adding still skips them.
      () => setInDeck(new Set()),
    );
  }, [cards, deckId, duplicateLane, refresh]);

  if (cards.length === 0 || !decks || decks.length === 0) return null;

  const offered = cards.flatMap((card, index) => inDeck?.has(index) ? [] : [{ card, index }]);
  const picked = offered.filter(({ index }) => !unpicked.has(index));
  const deck = decks.find((candidate) => candidate.id === deckId);

  const add = async () => {
    if (!deck || picked.length === 0) return;
    setAdding(true);
    setError(null);
    try {
      const result = await bulkCreateCards({ deckId: deck.id, rows: picked.map(({ card }) => card) });
      showToast("success", result.createdCount === 1
        ? `1 card added to ${deck.name}.`
        : `${result.createdCount} cards added to ${deck.name}.`);
      setRefresh((count) => count + 1);
    } catch (failure: unknown) {
      setError(getErrorMessage(failure));
    } finally {
      setAdding(false);
    }
  };

  return (
    <Disclosure
      title={cards.length === 1 ? "1 card from this slide" : `${cards.length} cards from this slide`}
      className="tw:bg-card"
    >
      <div className="tw:grid tw:gap-3 tw:p-4 tw:pt-1">
        <ul className="tw:m-0 tw:grid tw:list-none tw:gap-1 tw:p-0">
          {cards.map((card, index) => {
            const held = inDeck?.has(index) === true;
            return (
              <li key={`${index}-${card.question}`}>
                <label className={`tw:flex tw:min-h-11 tw:items-start tw:gap-3 tw:py-1 ${held ? "" : "tw:cursor-pointer"}`}>
                  <Checkbox
                    className="tw:mt-0.5"
                    checked={held || !unpicked.has(index)}
                    disabled={held || adding}
                    onCheckedChange={(on) => setUnpicked((current) => {
                      const next = new Set(current);
                      if (on) next.delete(index);
                      else next.add(index);
                      return next;
                    })}
                  />
                  <span className="tw:grid tw:min-w-0 tw:flex-1 tw:gap-0.5">
                    <span className="tw:font-medium tw:break-words">{card.question}</span>
                    <span className="tw:text-small tw:text-muted-foreground tw:break-words">{card.answer}</span>
                    {held ? <span className="tw:text-caption tw:text-muted-foreground">In {deck?.name ?? "the deck"}</span> : null}
                  </span>
                </label>
              </li>
            );
          })}
        </ul>
        {decks.length > 1 ? (
          <label className="tw:grid tw:min-w-0 tw:gap-1 tw:text-label tw:font-medium">
            <span>Deck</span>
            <NativeSelect value={deckId} disabled={adding} onChange={(event) => setDeckId(event.currentTarget.value)}>
              {decks.map((candidate) => (
                <option key={candidate.id} value={candidate.id}>{candidate.name}</option>
              ))}
            </NativeSelect>
          </label>
        ) : null}
        {error ? <InlineNotice kind="error">The cards could not be added: {error}</InlineNotice> : null}
        <div>
          <Button
            className="tw:rounded-full"
            disabled={adding || inDeck === null || picked.length === 0}
            onClick={() => void add()}
          >
            {offered.length === 0
              ? `All in ${deck?.name ?? "the deck"}`
              : decks.length > 1 ? "Add to deck" : `Add to ${deck?.name ?? "deck"}`}
          </Button>
        </div>
      </div>
    </Disclosure>
  );
}

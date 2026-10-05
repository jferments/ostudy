import { useRef, useState } from "react";
import type { SelectionCard } from "../../ipc/cards";
import { createOperationId, getErrorMessage } from "../../ipc/common";
import {
  saveSelectionAsDeck,
  type DeckSummary,
  type SaveSelectionAsDeckInput,
  type SaveSelectionAsDeckResult,
} from "../../ipc/decks";
import { Button } from "../../ui/primitives/Button";
import { TextField } from "../../ui/primitives/TextField";
import { Panel } from "../../ui/primitives/Panel";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Dialog } from "../../ui/overlays/Dialog";
import styles from "./browserPresentation";

interface PracticeResultsModalProps {
  cards: SelectionCard[];
  sourceLabel: string;
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeCards: (cardIds: string[]) => void;
  onClose: () => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

export function PracticeResultsModal({ cards, sourceLabel, refreshDecks, onPracticeCards, onClose, showToast }: PracticeResultsModalProps) {
  const [deckName, setDeckName] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [committed, setCommitted] = useState<SaveSelectionAsDeckResult | null>(null);
  const [reconciliationMessage, setReconciliationMessage] = useState<string | null>(null);
  const pendingSaveRef = useRef<SaveSelectionAsDeckInput | null>(null);
  const committedRef = useRef<SaveSelectionAsDeckResult | null>(null);
  const busyRef = useRef(false);

  function startTemporaryPractice(): void {
    if (cards.length === 0 || busyRef.current || committedRef.current) return;
    onPracticeCards(cards.map((card) => card.cardId));
    onClose();
  }

  async function savePermanentDeck(): Promise<void> {
    if (busyRef.current || committedRef.current) return;
    if (!pendingSaveRef.current && (deckName.trim() === "" || cards.length === 0)) return;
    busyRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);
    let result: SaveSelectionAsDeckResult;
    try {
      // An unconfirmed request is immutable. Retrying cannot allocate a new ID
      // or silently use a changed name/selection after a lost acknowledgement.
      if (!pendingSaveRef.current) {
        pendingSaveRef.current = {
          operationId: createOperationId(),
          name: deckName.trim(),
          description: `Created from a card selection in ${sourceLabel}.`,
          noteIds: [...new Set(cards.map((card) => card.noteId))].sort(),
        };
      }
      result = await saveSelectionAsDeck(pendingSaveRef.current);
      if (result.operationId !== pendingSaveRef.current.operationId) {
        throw new Error("The save result did not match this request. Retry to recover its original result.");
      }
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
      setIsSaving(false);
      busyRef.current = false;
      return;
    }

    // Commit is terminal BEFORE any fallible notification, refresh or navigation.
    committedRef.current = result;
    pendingSaveRef.current = null;
    setCommitted(result);
    const message = committedMessage(result);
    try { showToast("success", message); }
    catch { /* The visible committed receipt remains authoritative. */ }
    try {
      await refreshDecks();
    } catch (refreshError: unknown) {
      const warning = `The deck was created, but refreshing the deck list failed: ${getErrorMessage(refreshError)} Refresh the view; do not save the selection again.`;
      setReconciliationMessage(warning);
      try { showToast("info", warning); }
      catch { /* A notification failure cannot repeat or undo the mutation. */ }
    }
    try { onClose(); }
    catch (navigationError: unknown) {
      setReconciliationMessage(`The deck is saved. Closing this panel failed: ${getErrorMessage(navigationError)}`);
    }
    setIsSaving(false);
    busyRef.current = false;
  }

  const retrying = pendingSaveRef.current !== null;
  return (
    <Dialog title="Practice these cards"
      description={`${cards.length} selected ${cards.length === 1 ? "card" : "cards"} from ${sourceLabel}.`}
      width="medium" busy={isSaving} dismissible={!isSaving} onClose={() => { if (!busyRef.current) onClose(); }}
      actions={<Button variant="secondary" disabled={isSaving} onClick={onClose}>Close</Button>}>
      <div className={styles.screen} aria-busy={isSaving}>
        <Panel title="Temporary practice set"
          description="Start practicing these cards now without changing their current decks or creating a permanent deck.">
          <Button className="tw:justify-self-start" variant="primary" disabled={isSaving || committed !== null} onClick={startTemporaryPractice}>Start practice</Button>
        </Panel>
        <Panel title="Save as a permanent deck"
          description="Copies each selected note once, including its other study cards. The originals and their review histories remain in their current decks; copied cards start with fresh schedules.">
          <label className={styles.moveCopyDestination}><span>New deck name</span>
            <TextField value={deckName} disabled={isSaving || retrying || committed !== null} placeholder="Selected study set"
              onChange={(event) => { if (!pendingSaveRef.current && !committedRef.current) { setDeckName(event.currentTarget.value); setErrorMessage(null); } }} />
          </label>
          <Button className="tw:justify-self-start" variant="secondary" disabled={isSaving || committed !== null || (!retrying && deckName.trim() === "")} onClick={() => void savePermanentDeck()}>
            {isSaving ? "Creating deck…" : committed ? "Deck saved" : retrying ? "Retry save" : "Save as new deck"}
          </Button>
        </Panel>
        {committed ? <InlineNotice kind="info">{committedMessage(committed)}</InlineNotice> : null}
        {reconciliationMessage ? <InlineNotice kind="info">{reconciliationMessage}</InlineNotice> : null}
        {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
      </div>
    </Dialog>
  );
}

function committedMessage(result: SaveSelectionAsDeckResult): string {
  const count = result.deck.generatedCardCount;
  const duplicates = result.skippedDuplicates > 0
    ? ` ${result.skippedDuplicates} exact duplicate ${result.skippedDuplicates === 1 ? "note was" : "notes were"} omitted.` : "";
  return `Created “${result.deck.name}” with ${count} ${count === 1 ? "card" : "cards"}.${duplicates}`;
}

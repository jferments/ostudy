import { useState } from "react";

import { Modal } from "../../components/Modal";
import {
  transferCards,
  type SelectionCard,
} from "../../ipc/cards";
import {
  getErrorMessage,
} from "../../ipc/common";
import {
  createDeck,
  deleteDeck,
  type DeckSummary,
} from "../../ipc/decks";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { DialogActions } from "../../ui/overlays/Dialog";
import { ownedClassNames, sharedClassMaps } from "../../ui/styles/classOwnership";
import ownershipStyles from "./CardBrowserWorkspace.module.css";

const ownedStyleMaps = [...sharedClassMaps, ownershipStyles] as const;


interface PracticeResultsModalProps {
  cards: SelectionCard[];
  sourceLabel: string;
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeCards: (cardIds: string[]) => void;
  onClose: () => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

export function PracticeResultsModal({
  cards,
  sourceLabel,
  refreshDecks,
  onPracticeCards,
  onClose,
  showToast,
}: PracticeResultsModalProps) {
  const [deckName, setDeckName] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  function startTemporaryPractice(): void {
    if (cards.length === 0 || isSaving) return;
    onPracticeCards(cards.map((card) => card.cardId));
    onClose();
  }

  async function savePermanentDeck(): Promise<void> {
    const name = deckName.trim();
    if (name === "" || cards.length === 0 || isSaving) return;
    setIsSaving(true);
    setErrorMessage(null);
    let createdDeck: DeckSummary | null = null;
    try {
      createdDeck = await createDeck({
        name,
        description: `Created from a card selection in ${sourceLabel}.`,
      });
      const result = await transferCards({
        noteIds: cards.map((card) => card.noteId),
        targetDeckId: createdDeck.id,
        mode: "copy",
        skipDuplicates: true,
      });
      if (result.affectedCount === 0) {
        throw new Error("No cards could be copied into the new deck.");
      }
      await refreshDecks();
      const duplicateText =
        result.skippedDuplicates > 0
          ? ` ${result.skippedDuplicates} exact ${
              result.skippedDuplicates === 1 ? "duplicate was" : "duplicates were"
            } omitted.`
          : "";
      showToast(
        "success",
        `Created “${createdDeck.name}” with ${result.affectedCount} ${
          result.affectedCount === 1 ? "card" : "cards"
        }.${duplicateText}`,
      );
      onClose();
    } catch (error: unknown) {
      if (createdDeck) {
        try {
          await deleteDeck(createdDeck.id);
        } catch (rollbackError: unknown) {
          console.error(
            "Unable to remove incomplete filtered deck:",
            getErrorMessage(rollbackError),
          );
        }
      }
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <Modal
      title="Practice these cards"
      description={`${cards.length} selected ${cards.length === 1 ? "card" : "cards"} from ${sourceLabel}.`}
      width="medium"
      onClose={() => {
        if (!isSaving) onClose();
      }}
    >
      <div className={ownedClassNames(ownedStyleMaps, "practice-results-form")} aria-busy={isSaving}>
        <section className={ownedClassNames(ownedStyleMaps, "practice-result-option")}>
          <div>
            <h3>Temporary practice set</h3>
            <p>
              Start practicing these cards now without changing their current decks or creating a
              permanent deck.
            </p>
          </div>
          <Button
            type="button"
            variant="primary"
            disabled={isSaving}
            onClick={startTemporaryPractice}
          >
            Start practice
          </Button>
        </section>

        <section className={ownedClassNames(ownedStyleMaps, "practice-result-option saved-result-option")}>
          <div>
            <h3>Save as a permanent deck</h3>
            <p>
              Copies the selected cards into a new deck. The originals and their review histories
              remain in their current decks; copied cards start with fresh schedules.
            </p>
          </div>
          <label>
            <span>New deck name</span>
            <input
              value={deckName}
              disabled={isSaving}
              placeholder="Selected study set"
              onChange={(event) => {
                setDeckName(event.currentTarget.value);
                setErrorMessage(null);
              }}
            />
          </label>
          <Button
            type="button"
            variant="secondary"
            disabled={isSaving || deckName.trim() === ""}
            onClick={() => void savePermanentDeck()}
          >
            {isSaving ? "Creating deck…" : "Save as new deck"}
          </Button>
        </section>

        {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
        <DialogActions>
          <Button
            type="button"
            variant="secondary"
            disabled={isSaving}
            onClick={onClose}
          >
            Close
          </Button>
        </DialogActions>
      </div>
    </Modal>
  );
}
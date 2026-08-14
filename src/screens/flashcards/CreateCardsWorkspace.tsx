import { useMemo, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import type { DeckSummary } from "../../ipc/decks";
import type { GenerateTemplateCardsResult, CardTemplate } from "../../ipc/templates";
import { Artwork } from "../../ui/artwork/Artwork";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { Surface } from "../../ui/primitives/Surface";
import { CardEditorScreen } from "../CardEditorScreen";
import { BulkEntryModal } from "../cards/BulkEntryModal";
import styles from "./FlashcardsWorkspace.module.css";

export function CreateCardsWorkspace({
  decks,
  refreshDecks,
  onDirtyChange,
  showToast,
}: {
  decks: DeckSummary[];
  refreshDecks: () => Promise<DeckSummary[]>;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}) {
  const activeDecks = useMemo(() => decks.filter((deck) => deck.archivedAt === null), [decks]);
  const [deckId, setDeckId] = useState(activeDecks[0]?.id ?? "");
  const [editorOpen, setEditorOpen] = useState(false);
  const [bulkOpen, setBulkOpen] = useState(false);
  const deck = activeDecks.find((candidate) => candidate.id === deckId) ?? activeDecks[0] ?? null;

  if (editorOpen && deck) {
    return (
      <CardEditorScreen
        deck={deck}
        decks={activeDecks}
        card={null}
        onDirtyChange={onDirtyChange}
        onCancel={() => {
          onDirtyChange(false);
          setEditorOpen(false);
        }}
        onSaved={async (_saved, continueAdding) => {
          onDirtyChange(false);
          showToast("success", "Flashcard saved.");
          await refreshDecks();
          if (!continueAdding) setEditorOpen(false);
        }}
        onTemplateGenerated={async (
          result: GenerateTemplateCardsResult,
          continueAdding: boolean,
          template: CardTemplate,
        ) => {
          onDirtyChange(false);
          showToast(
            result.createdCount > 0 ? "success" : "info",
            `Generated ${result.createdCount} ${result.createdCount === 1 ? "card" : "cards"} from “${template.name}”.`,
          );
          await refreshDecks();
          if (!continueAdding) setEditorOpen(false);
        }}
      />
    );
  }

  return (
    <div className={styles.createLayout} data-testid="create-cards-workspace">
      <Surface level={2} className={styles.createHero} data-testid="create-cards-hero">
        <Artwork slot="empty.deck" />
        <div>
          <span className={styles.workspaceEyebrow}>Create cards</span>
          <h2>Add study material without leaving the Flashcards workspace</h2>
          <p>Create one rich flashcard, generate cards from a reusable template, or enter many cards in a compact grid.</p>
        </div>
      </Surface>

      <Surface level={2} className={styles.createControls}>
        <label className={styles.deckSelect}>
          <span>Destination deck</span>
          <select value={deck?.id ?? ""} disabled={!deck} onChange={(event) => setDeckId(event.currentTarget.value)}>
            {activeDecks.map((candidate) => <option key={candidate.id} value={candidate.id}>{candidate.name}</option>)}
          </select>
        </label>
        {deck ? (
          <div className={styles.createActions}>
            <Button variant="primary" size="large" onClick={() => setEditorOpen(true)}><Icon name="plus" size={20} /> Create one card</Button>
            <Button data-testid="bulk-add-cards" size="large" onClick={() => setBulkOpen(true)}><Icon name="cards" size={20} /> Bulk add</Button>
          </div>
        ) : (
          <p>Create an active deck before adding flashcards.</p>
        )}
      </Surface>

      {bulkOpen ? (
        <BulkEntryModal
          decks={activeDecks}
          onClose={() => setBulkOpen(false)}
          onSaved={async () => {
            setBulkOpen(false);
            await refreshDecks();
            showToast("success", "Bulk flashcards saved.");
          }}
          showToast={showToast}
        />
      ) : null}
    </div>
  );
}

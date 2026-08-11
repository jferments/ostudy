import { useState } from "react";

import type { ShowToast } from "../components/Toast";
import type { DeckSummary } from "../ipc/decks";
import type { AppSettings } from "../ipc/settings";
import { CardsScreen } from "./CardsScreen";
import { DecksScreen } from "./DecksScreen";
import { BulkEntryModal } from "./cards/BulkEntryModal";

interface DecksWorkspaceProps {
  decks: DeckSummary[];
  settings: AppSettings;
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeDeck: (deckId: string) => void | Promise<void>;
  onPracticeCards: (cardIds: string[]) => void | Promise<void>;
  onManageTemplates: () => void | Promise<void>;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

type DeckWorkspaceView = "library" | "detail" | "editor";

export function DecksWorkspace({
  decks,
  settings,
  refreshDecks,
  onPracticeDeck,
  onPracticeCards,
  onManageTemplates,
  onDirtyChange,
  showToast,
}: DecksWorkspaceProps) {
  const [bulkOpen, setBulkOpen] = useState(false);
  const [deckView, setDeckView] = useState<DeckWorkspaceView>("library");

  return (
    <div className="decks-workspace">
      <DecksScreen
        decks={decks}
        settings={settings}
        refreshDecks={refreshDecks}
        onPracticeDeck={onPracticeDeck}
        onManageTemplates={onManageTemplates}
        onBulkAdd={() => setBulkOpen(true)}
        onViewChange={setDeckView}
        onDirtyChange={onDirtyChange}
        showToast={showToast}
      />

      {deckView === "library" ? (
        <section className="decks-global-search" aria-labelledby="all-cards-search-heading">
          <div className="decks-global-search-heading">
            <div>
              <span className="eyebrow">CARD LIBRARY</span>
              <h2 id="all-cards-search-heading">Search and filter cards</h2>
              <p>Search every active deck or narrow the results to selected decks.</p>
            </div>
          </div>
          <CardsScreen
            embedded
            decks={decks}
            refreshDecks={refreshDecks}
            onPracticeCards={onPracticeCards}
            onDirtyChange={onDirtyChange}
            showToast={showToast}
          />
        </section>
      ) : null}

      {bulkOpen ? (
        <BulkEntryModal
          decks={decks}
          onClose={() => setBulkOpen(false)}
          onSaved={async () => {
            setBulkOpen(false);
            await refreshDecks();
          }}
          showToast={showToast}
        />
      ) : null}
    </div>
  );
}

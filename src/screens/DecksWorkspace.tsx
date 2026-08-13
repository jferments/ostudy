import { useEffect, useState } from "react";

import type { DeckOpenRequest } from "../App";
import type { ShowToast } from "../components/Toast";
import type { DeckSummary } from "../ipc/decks";
import type { AppSettings } from "../ipc/settings";
import { registerPlatformBackHandler } from "../platform/backNavigation";
import { CardsScreen } from "./CardsScreen";
import { DecksScreen } from "./DecksScreen";
import { BulkEntryModal } from "./cards/BulkEntryModal";

interface DecksWorkspaceProps {
  decks: DeckSummary[];
  settings: AppSettings;
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeDeck: (deckId: string, origin: "library" | "detail") => void | Promise<void>;
  openDeckRequest: DeckOpenRequest | null;
  onOpenDeckRequestConsumed: () => void;
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
  openDeckRequest,
  onOpenDeckRequestConsumed,
  onManageTemplates,
  onDirtyChange,
  showToast,
}: DecksWorkspaceProps) {
  const [bulkOpen, setBulkOpen] = useState(false);
  const [deckView, setDeckView] = useState<DeckWorkspaceView>("library");

  useEffect(() => {
    if (deckView === "library") return;
    return registerPlatformBackHandler(() => {
      // DecksScreen and CardEditorScreen already centralize their safe exit
      // behavior in the visible Back controls (including dirty-draft prompts).
      // Reusing that control keeps Android hardware Back and pointer/keyboard
      // navigation on the exact same code path instead of duplicating discard
      // and durable-draft semantics in this workspace wrapper.
      const back = document.querySelector<HTMLButtonElement>(
        ".decks-workspace .back-button:not([disabled])",
      );
      if (!back) return false;
      back.click();
      return true;
    });
  }, [deckView]);

  return (
    <div className="decks-workspace">
      <DecksScreen
        decks={decks}
        settings={settings}
        refreshDecks={refreshDecks}
        onPracticeDeck={onPracticeDeck}
        openDeckRequest={openDeckRequest}
        onOpenDeckRequestConsumed={onOpenDeckRequestConsumed}
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

import { useRef } from "react";

import type { DeckOpenRequest } from "../../App";
import type { ShowToast } from "../../components/Toast";
import type { DeckSummary } from "../../ipc/decks";
import type { AppSettings } from "../../ipc/settings";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { PerformanceProfiler } from "../../testing/PerformanceProfiler";
import { DecksScreen } from "../DecksScreen";
import { DeckLibraryRoot } from "./deck-library/DeckLibraryRoot";
import { useDeckLibraryController } from "./deck-library/useDeckLibraryController";

interface DeckLibraryWorkspaceProps {
  decks: DeckSummary[];
  settings: AppSettings;
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeDeck: (deckId: string, origin: "library" | "detail") => void | Promise<void>;
  openDeckRequest: DeckOpenRequest | null;
  onOpenDeck: (deckId: string) => void;
  onReturnToLibrary: () => void;
  onManageTemplates: () => void | Promise<void>;
  onCardDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

export function DeckLibraryWorkspace({
  decks,
  settings,
  refreshDecks,
  onPracticeDeck,
  openDeckRequest,
  onOpenDeck,
  onReturnToLibrary,
  onManageTemplates,
  onCardDirtyChange,
  showToast,
}: DeckLibraryWorkspaceProps) {
  const detailOpenedRef = useRef(false);
  const confirm = useConfirm();
  const controller = useDeckLibraryController({
    decks,
    refreshDecks,
    onOpenDeck,
    showToast,
    confirm,
  });

  if (openDeckRequest) {
    return (
      <DecksScreen
        key={openDeckRequest.requestId}
        decks={decks}
        settings={settings}
        refreshDecks={refreshDecks}
        onPracticeDeck={onPracticeDeck}
        openDeckRequest={openDeckRequest}
        onOpenDeckRequestConsumed={() => undefined}
        onManageTemplates={onManageTemplates}
        onViewChange={(view) => {
          if (view === "detail" || view === "editor") detailOpenedRef.current = true;
          if (view === "library" && detailOpenedRef.current) {
            detailOpenedRef.current = false;
            onReturnToLibrary();
            void controller.refreshLibraryAfterDetail();
          }
        }}
        onDirtyChange={onCardDirtyChange}
        showToast={showToast}
      />
    );
  }

  return (
    <PerformanceProfiler id="deck-library">
      <DeckLibraryRoot
        controller={controller}
        onPracticeDeck={(deckId) => onPracticeDeck(deckId, "library")}
      />
    </PerformanceProfiler>
  );
}

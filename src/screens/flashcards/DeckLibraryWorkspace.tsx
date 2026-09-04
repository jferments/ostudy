import { useRef } from "react";

import type { DeckOpenRequest } from "../../App";
import type { ShowToast } from "../../components/Toast";
import type { DeckSummary } from "../../ipc/decks";
import type { AppSettings } from "../../ipc/settings";
import type { SelectedPracticeRequest } from "../../ipc/practice";
import type { TransientDeckSelectionEntry } from "../../navigation/flashcardsRoutes";
import { PerformanceProfiler } from "../../testing/PerformanceProfiler";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { DeckDetailRoute } from "./DeckDetailRoute";
import { DeckLibraryRoot } from "./deck-library/DeckLibraryRoot";
import { useDeckLibraryController } from "./deck-library/useDeckLibraryController";

interface DeckLibraryWorkspaceProps {
  decks: DeckSummary[];
  settings: AppSettings;
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeDeck: (deckId: string) => void | Promise<void>;
  onPracticeSelected: (
    request: SelectedPracticeRequest,
    originSelection: readonly TransientDeckSelectionEntry[],
  ) => void | Promise<void>;
  restoredSelection?: readonly TransientDeckSelectionEntry[];
  openDeckRequest: DeckOpenRequest | null;
  onOpenDeck: (
    deckId: string,
    returnDeckSelection?: readonly TransientDeckSelectionEntry[],
  ) => void;
  onReturnToLibrary: () => void | Promise<void>;
  onManageTemplates: () => void | Promise<void>;
  onCardDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

export function DeckLibraryWorkspace({
  decks,
  settings,
  refreshDecks,
  onPracticeDeck,
  onPracticeSelected,
  restoredSelection = [],
  openDeckRequest,
  onOpenDeck,
  onReturnToLibrary,
  onCardDirtyChange,
  showToast,
}: DeckLibraryWorkspaceProps) {
  const controllerSelectionRef = useRef<TransientDeckSelectionEntry[]>([]);
  const confirm = useConfirm();
  const controller = useDeckLibraryController({
    decks,
    refreshDecks,
    onOpenDeck: (deckId) => onOpenDeck(deckId, controllerSelectionRef.current),
    showToast,
    confirm,
    restoredSelection,
  });
  controllerSelectionRef.current = controllerSelectionEntries(controller.selection);

  if (openDeckRequest) {
    return (
      <DeckDetailRoute
        key={openDeckRequest.requestId}
        decks={decks}
        refreshDecks={refreshDecks}
        onPracticeDeck={onPracticeDeck}
        openRequest={openDeckRequest}
        onReturnToLibrary={onReturnToLibrary}
        onDirtyChange={onCardDirtyChange}
        showToast={showToast}
      />
    );
  }

  return (
    <PerformanceProfiler id="deck-library">
      <DeckLibraryRoot
        controller={controller}
        onPracticeDeck={onPracticeDeck}
        decks={decks}
        practiceShuffle={settings.practiceShuffle}
        onPracticeSelected={(request) =>
          onPracticeSelected(
            request,
            controller.selectionAnalysis.decks.map((deck) => ({
              id: deck.id,
              revision: deck.revision,
            })),
          )
        }
      />
    </PerformanceProfiler>
  );
}

function controllerSelectionEntries(
  selection: ReturnType<typeof useDeckLibraryController>["selection"],
): TransientDeckSelectionEntry[] {
  return [...selection.entries.values()].map(({ id, revision }) => ({ id, revision }));
}

import type { ShowToast } from "../components/Toast";
import type { DeckSummary } from "../ipc/decks";
import { CardBrowserWorkspace } from "./cards/CardBrowserWorkspace";

interface CardsScreenProps {
  decks: DeckSummary[];
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeCards: (cardIds: string[]) => void | Promise<void>;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
  embedded?: boolean;
}

/**
 * Search/Filter is now a thin route wrapper around the same card browser used
 * inside an open deck. Filtering, pagination, card rows, authoring, selection,
 * and bulk operations all live in CardBrowserWorkspace.
 */
export function CardsScreen({
  decks,
  refreshDecks,
  onPracticeCards,
  onDirtyChange,
  showToast,
  embedded = false,
}: CardsScreenProps) {
  return (
    <CardBrowserWorkspace
      decks={decks}
      refreshDecks={refreshDecks}
      onPracticeCards={onPracticeCards}
      onDirtyChange={onDirtyChange}
      showToast={showToast}
      showFilters
      showPageHeader={!embedded}
    />
  );
}

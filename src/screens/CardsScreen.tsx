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
 * Search/Filter is a thin route wrapper around the same card browser used
 * inside an open deck. The shared browser owns queries, pagination, rows,
 * selection, editing, and bulk card operations while this route explicitly
 * selects the full Search/Filter control surface and omits deck-only authoring.
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
      controls="search"
      showPageHeader={!embedded}
    />
  );
}
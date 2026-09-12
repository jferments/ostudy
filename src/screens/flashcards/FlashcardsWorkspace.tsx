import { useEffect, useLayoutEffect, useRef } from "react";

import type { DeckOpenRequest } from "../../App";
import type { ShowToast } from "../../components/Toast";
import type { DeckSummary } from "../../ipc/decks";
import type { AppSettings } from "../../ipc/settings";
import type { SelectedPracticeRequest } from "../../ipc/practice";
import type {
  AnalyticsSearchLaunch,
  FlashcardsWorkspaceName,
  TransientDeckSelectionEntry,
} from "../../navigation/flashcardsRoutes";
import { registerPlatformBackHandler } from "../../platform/backNavigation";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { RouteTransition } from "../../ui/motion/Motion";
import { PageHeader, PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { CardsScreen } from "../CardsScreen";
import { CardPracticeProvider } from "../cards/CardPracticeContext";
import { DeckLibraryWorkspace } from "./DeckLibraryWorkspace";
import { WorkspaceTabs } from "../../ui/primitives/Tabs";

interface FlashcardsWorkspaceProps {
  workspace: FlashcardsWorkspaceName;
  navigationKey: number;
  onWorkspaceChange: (workspace: FlashcardsWorkspaceName) => void | Promise<void>;
  decks: DeckSummary[];
  settings: AppSettings;
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeDeck: (deckId: string) => void | Promise<void>;
  onPracticeSelected: (
    request: SelectedPracticeRequest,
    originSelection: readonly TransientDeckSelectionEntry[],
  ) => void | Promise<void>;
  restoredSelection?: readonly TransientDeckSelectionEntry[];
  analyticsSearchLaunch?: AnalyticsSearchLaunch;
  openDeckRequest: DeckOpenRequest | null;
  onOpenDeck: (
    deckId: string,
    returnDeckSelection?: readonly TransientDeckSelectionEntry[],
  ) => void;
  onReturnToLibrary: () => void | Promise<void>;
  onPracticeCards: (cardIds: string[]) => void | Promise<void>;
  onCardDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

const WORKSPACES: ReadonlyArray<{
  value: "decks" | "search";
  label: string;
  compactLabel: string;
  icon: IconName;
}> = [
  { value: "decks", label: "Decks", compactLabel: "Decks", icon: "cards" },
  {
    value: "search",
    label: "Search/Filter",
    compactLabel: "Search",
    icon: "search",
  },
];

export function FlashcardsWorkspace({
  workspace,
  navigationKey,
  onWorkspaceChange,
  decks,
  settings,
  refreshDecks,
  onPracticeDeck,
  onPracticeSelected,
  restoredSelection,
  analyticsSearchLaunch,
  onPracticeCards,
  openDeckRequest,
  onOpenDeck,
  onReturnToLibrary,
  onCardDirtyChange,
  showToast,
}: FlashcardsWorkspaceProps) {
  const workspaceChangeRef = useRef(onWorkspaceChange);
  const visibleWorkspace: "decks" | "search" = workspace === "search" ? "search" : "decks";

  useLayoutEffect(() => {
    workspaceChangeRef.current = onWorkspaceChange;
  }, [onWorkspaceChange]);

  useEffect(() => {
    if (visibleWorkspace === "decks") return;
    return registerPlatformBackHandler(() => {
      void workspaceChangeRef.current("decks");
      return true;
    });
  }, [visibleWorkspace]);

  return (
    <PageScaffold width="full" className="tw:@container/flashcards-workspace">
      <PageHeader title="Flashcards" icon="cards" compact />

      <WorkspaceTabs value={visibleWorkspace} onValueChange={(next) => void onWorkspaceChange(next)} ariaLabel="Flashcards workspaces" tourAnchor="flashcards.workspaces" tabs={WORKSPACES.map((item) => ({ ...item, icon: <Icon name={item.icon} size={18} /> }))}>
      <RouteTransition
        routeKey={`${visibleWorkspace}:${navigationKey}`}
        relation="sibling"
        className="tw:min-w-0"
      >
        <CardPracticeProvider onPracticeCards={onPracticeCards}>
          {visibleWorkspace === "decks" ? (
            <DeckLibraryWorkspace
              key={navigationKey}
              decks={decks}
              settings={settings}
              refreshDecks={refreshDecks}
              onPracticeDeck={onPracticeDeck}
              onPracticeSelected={onPracticeSelected}
              restoredSelection={restoredSelection}
              openDeckRequest={openDeckRequest}
              onOpenDeck={onOpenDeck}
              onReturnToLibrary={onReturnToLibrary}
              onCardDirtyChange={onCardDirtyChange}
              showToast={showToast}
            />
          ) : null}
          {visibleWorkspace === "search" ? (
            <CardsScreen
              key={navigationKey}
              decks={decks}
              refreshDecks={refreshDecks}
              onPracticeCards={onPracticeCards}
              onDirtyChange={onCardDirtyChange}
              showToast={showToast}
              analyticsSearchLaunch={analyticsSearchLaunch}
            />
          ) : null}
        </CardPracticeProvider>
      </RouteTransition>
      </WorkspaceTabs>
    </PageScaffold>
  );
}

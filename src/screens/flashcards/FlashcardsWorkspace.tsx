import { useEffect, useLayoutEffect, useRef, type KeyboardEvent } from "react";

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
import styles from "./FlashcardsWorkspace.module.css";

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
  const buttonsRef = useRef<Array<HTMLButtonElement | null>>([]);
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

  function activateAt(index: number): void {
    const item = WORKSPACES[index];
    if (!item) return;
    void onWorkspaceChange(item.value);
    buttonsRef.current[index]?.focus();
  }

  function handleTabKeyDown(event: KeyboardEvent<HTMLButtonElement>, index: number): void {
    let next: number | null = null;
    if (event.key === "ArrowRight") next = (index + 1) % WORKSPACES.length;
    if (event.key === "ArrowLeft") next = (index - 1 + WORKSPACES.length) % WORKSPACES.length;
    if (event.key === "Home") next = 0;
    if (event.key === "End") next = WORKSPACES.length - 1;
    if (next === null) return;
    event.preventDefault();
    activateAt(next);
  }

  return (
    <PageScaffold width="full" className={styles.workspace}>
      <PageHeader title="Flashcards" icon="cards" compact />

      <div className={styles.tabScroller}>
        <div
          className={styles.tabList}
          role="tablist"
          aria-label="Flashcards workspaces"
          data-tour-anchor="flashcards.workspaces"
        >
          {WORKSPACES.map((item, index) => {
            const selected = item.value === visibleWorkspace;
            return (
              <button
                key={item.value}
                ref={(node) => {
                  buttonsRef.current[index] = node;
                }}
                type="button"
                role="tab"
                aria-label={item.label}
                aria-selected={selected}
                tabIndex={selected ? 0 : -1}
                className={styles.tab}
                data-selected={selected}
                onClick={() => void onWorkspaceChange(item.value)}
                onKeyDown={(event) => handleTabKeyDown(event, index)}
              >
                <Icon name={item.icon} size={18} />
                <span className={styles.tabLabel} data-compact-label={item.compactLabel}>
                  {item.label}
                </span>
              </button>
            );
          })}
        </div>
      </div>

      <RouteTransition
        routeKey={`${visibleWorkspace}:${navigationKey}`}
        relation="sibling"
        className={styles.panel}
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
    </PageScaffold>
  );
}

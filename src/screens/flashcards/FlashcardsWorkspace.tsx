import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";

import type { DeckOpenRequest } from "../../App";
import type { ShowToast } from "../../components/Toast";
import type { DeckSummary } from "../../ipc/decks";
import type { AppSettings } from "../../ipc/settings";
import type {
  FlashcardsRootRequest,
  FlashcardsRouteState,
  FlashcardsWorkspaceName,
} from "../../navigation/flashcardsRoutes";
import { registerPlatformBackHandler } from "../../platform/backNavigation";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { RouteTransition } from "../../ui/motion/Motion";
import { PageHeader, PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { CardsScreen } from "../CardsScreen";
import { TemplatesScreen } from "../TemplatesScreen";
import { BulkEntryModal } from "../cards/BulkEntryModal";
import { DeckLibraryWorkspace } from "./DeckLibraryWorkspace";
import { TransferWorkspace } from "./TransferWorkspace";
import styles from "./FlashcardsWorkspace.module.css";

interface FlashcardsWorkspaceProps {
  workspace: FlashcardsWorkspaceName;
  rootRequest: FlashcardsRootRequest | null;
  onRouteChange: (route: FlashcardsRouteState) => void;
  onWorkspaceChange: (workspace: FlashcardsWorkspaceName) => void | Promise<void>;
  decks: DeckSummary[];
  settings: AppSettings;
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeDeck: (deckId: string, origin: "library" | "detail") => void | Promise<void>;
  openDeckRequest: DeckOpenRequest | null;
  onOpenDeckRequestConsumed: () => void;
  onPracticeCards: (cardIds: string[]) => void | Promise<void>;
  onCardDirtyChange: (dirty: boolean) => void;
  onTemplateDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

const WORKSPACES: ReadonlyArray<{
  value: FlashcardsWorkspaceName;
  label: string;
  compactLabel: string;
  icon: IconName;
}> = [
  { value: "decks", label: "Decks", compactLabel: "Decks", icon: "cards" },
  { value: "templates", label: "Templates", compactLabel: "Templates", icon: "template" },
  { value: "search", label: "Search", compactLabel: "Search", icon: "search" },
  { value: "transfer", label: "Import/Export", compactLabel: "Transfer", icon: "import" },
];

const LIBRARY_ROUTE: FlashcardsRouteState = {
  workspace: "decks",
  deckView: "library",
  deckId: null,
};

export function FlashcardsWorkspace({
  workspace,
  rootRequest,
  onRouteChange,
  onWorkspaceChange,
  decks,
  settings,
  refreshDecks,
  onPracticeDeck,
  onPracticeCards,
  openDeckRequest,
  onOpenDeckRequestConsumed,
  onCardDirtyChange,
  onTemplateDirtyChange,
  showToast,
}: FlashcardsWorkspaceProps) {
  const [, setDeckRoute] = useState<FlashcardsRouteState>(LIBRARY_ROUTE);
  const [bulkDeckId, setBulkDeckId] = useState<string | null>(null);
  const buttonsRef = useRef<Array<HTMLButtonElement | null>>([]);
  const workspaceChangeRef = useRef(onWorkspaceChange);
  const effectiveRootRequestId = rootRequest?.workspace === workspace ? rootRequest.requestId : 0;

  useLayoutEffect(() => {
    workspaceChangeRef.current = onWorkspaceChange;
  }, [onWorkspaceChange]);

  const reportDeckRoute = useCallback(
    (route: FlashcardsRouteState) => {
      setDeckRoute(route);
      onRouteChange(route);
    },
    [onRouteChange],
  );

  useEffect(() => {
    if (workspace === "decks") return;
    onRouteChange({ workspace, deckView: "library", deckId: null });
  }, [onRouteChange, workspace]);

  useEffect(() => {
    if (workspace === "decks") return;
    return registerPlatformBackHandler(() => {
      void workspaceChangeRef.current("decks");
      return true;
    });
  }, [workspace]);

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

  const bulkDeck = decks.find((deck) => deck.id === bulkDeckId && deck.archivedAt === null) ?? null;

  return (
    <PageScaffold width="full" className={`flashcards-workspace ${styles.workspace}`}>
      <PageHeader title="Flashcards" icon="cards" compact />

      <div className={styles.tabScroller}>
        <div
          className={styles.tabList}
          role="tablist"
          aria-label="Flashcards workspaces"
          data-tour-anchor="flashcards.workspaces"
        >
          {WORKSPACES.map((item, index) => {
            const selected = item.value === workspace;
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
        routeKey={`${workspace}:${workspace === "decks" ? 0 : effectiveRootRequestId}`}
        relation="sibling"
        className={styles.panel}
      >
        {workspace === "decks" ? (
          <DeckLibraryWorkspace
            decks={decks}
            settings={settings}
            refreshDecks={refreshDecks}
            onPracticeDeck={onPracticeDeck}
            openDeckRequest={openDeckRequest}
            onOpenDeckRequestConsumed={onOpenDeckRequestConsumed}
            onManageTemplates={() => onWorkspaceChange("templates")}
            onBulkAddDeck={setBulkDeckId}
            rootRequestId={effectiveRootRequestId}
            onRouteChange={reportDeckRoute}
            onCardDirtyChange={onCardDirtyChange}
            showToast={showToast}
          />
        ) : null}
        {workspace === "templates" ? (
          <TemplatesScreen
            onBack={() => onWorkspaceChange("decks")}
            onDirtyChange={onTemplateDirtyChange}
            showToast={showToast}
          />
        ) : null}
        {workspace === "search" ? (
          <CardsScreen
            decks={decks}
            refreshDecks={refreshDecks}
            onPracticeCards={onPracticeCards}
            onDirtyChange={onCardDirtyChange}
            showToast={showToast}
          />
        ) : null}
        {workspace === "transfer" ? (
          <TransferWorkspace decks={decks} refreshDecks={refreshDecks} showToast={showToast} />
        ) : null}
      </RouteTransition>

      {bulkDeck ? (
        <BulkEntryModal
          key={bulkDeck.id}
          decks={decks}
          defaultDeckId={bulkDeck.id}
          destinationMode="fixed"
          onClose={() => setBulkDeckId(null)}
          onSaved={async () => {
            await refreshDecks();
            setBulkDeckId(null);
          }}
          showToast={showToast}
        />
      ) : null}
    </PageScaffold>
  );
}

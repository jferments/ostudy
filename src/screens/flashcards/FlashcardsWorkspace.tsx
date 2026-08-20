import { useEffect, useRef, type KeyboardEvent } from "react";

import type { ShowToast } from "../../components/Toast";
import type { DeckSummary } from "../../ipc/decks";
import type { AppSettings } from "../../ipc/settings";
import type {
  FlashcardsDeckView,
  FlashcardsRoute,
  FlashcardsWorkspaceName,
} from "../../navigation/flashcardsRoutes";
import { registerPlatformBackHandler } from "../../platform/backNavigation";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { RouteTransition } from "../../ui/motion/Motion";
import { PageHeader, PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { CardsScreen } from "../CardsScreen";
import { TemplatesScreen } from "../TemplatesScreen";
import { DeckLibraryWorkspace } from "./DeckLibraryWorkspace";
import { TransferWorkspace } from "./TransferWorkspace";
import styles from "./FlashcardsWorkspace.module.css";

interface FlashcardsWorkspaceProps {
  route: FlashcardsRoute;
  onWorkspaceChange: (workspace: FlashcardsWorkspaceName) => void | Promise<void>;
  onDeckViewChange: (view: FlashcardsDeckView, deckId: string | null) => void;
  decks: DeckSummary[];
  settings: AppSettings;
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeDeck: (deckId: string, origin: "library" | "detail") => void | Promise<void>;
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

export function FlashcardsWorkspace({
  route,
  onWorkspaceChange,
  onDeckViewChange,
  decks,
  settings,
  refreshDecks,
  onPracticeDeck,
  onPracticeCards,
  onCardDirtyChange,
  onTemplateDirtyChange,
  showToast,
}: FlashcardsWorkspaceProps) {
  const buttonsRef = useRef<Array<HTMLButtonElement | null>>([]);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const workspaceChangeRef = useRef(onWorkspaceChange);
  workspaceChangeRef.current = onWorkspaceChange;
  const workspace = route.workspace;
  const deckNested = workspace === "decks" && route.view !== "library";

  useEffect(() => {
    if (workspace === "decks" && !deckNested) return;
    return registerPlatformBackHandler(() => {
      if (workspace === "decks") {
        const back = panelRef.current?.querySelector<HTMLButtonElement>(
          ".back-button:not([disabled])",
        );
        if (!back) return false;
        back.click();
        return true;
      }
      void workspaceChangeRef.current("decks");
      return true;
    });
  }, [deckNested, workspace]);

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

  const routeKey = workspace === "decks" ? workspace : `${workspace}:${route.requestId}`;

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

      <div ref={panelRef} className={styles.panelContainer}>
        <RouteTransition routeKey={routeKey} relation="sibling" className={styles.panel}>
          {route.workspace === "decks" ? (
            <DeckLibraryWorkspace
              route={route}
              onViewChange={onDeckViewChange}
              decks={decks}
              settings={settings}
              refreshDecks={refreshDecks}
              onPracticeDeck={onPracticeDeck}
              onManageTemplates={() => onWorkspaceChange("templates")}
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
            <TransferWorkspace
              decks={decks}
              refreshDecks={refreshDecks}
              showToast={showToast}
            />
          ) : null}
        </RouteTransition>
      </div>
    </PageScaffold>
  );
}

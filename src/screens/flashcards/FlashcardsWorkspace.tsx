import {
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
import type { FlashcardsWorkspaceName } from "../../navigation/flashcardsRoutes";
import { registerPlatformBackHandler } from "../../platform/backNavigation";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { RouteTransition } from "../../ui/motion/Motion";
import { PageHeader, PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { CardsScreen } from "../CardsScreen";
import { TemplatesScreen } from "../TemplatesScreen";
import { CreateCardsWorkspace } from "./CreateCardsWorkspace";
import { DeckLibraryWorkspace } from "./DeckLibraryWorkspace";
import { TransferWorkspace } from "./TransferWorkspace";
import styles from "./FlashcardsWorkspace.module.css";

interface FlashcardsWorkspaceProps {
  workspace: FlashcardsWorkspaceName;
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
  { value: "create", label: "Create Cards", compactLabel: "Create", icon: "plus" },
];

export function FlashcardsWorkspace({
  workspace,
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
  const [deckNested, setDeckNested] = useState(false);
  const buttonsRef = useRef<Array<HTMLButtonElement | null>>([]);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const workspaceChangeRef = useRef(onWorkspaceChange);

  useLayoutEffect(() => {
    workspaceChangeRef.current = onWorkspaceChange;
  }, [onWorkspaceChange]);

  useLayoutEffect(() => {
    if (workspace !== "decks" || !panelRef.current) {
      setDeckNested(false);
      return;
    }
    const panel = panelRef.current;
    const update = () => {
      setDeckNested(Boolean(panel.querySelector(".back-button:not([disabled])")));
    };
    update();
    const observer = new MutationObserver(update);
    observer.observe(panel, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["disabled"],
    });
    return () => observer.disconnect();
  }, [workspace]);

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
                aria-selected={selected}
                tabIndex={selected ? 0 : -1}
                className={styles.tab}
                data-selected={selected}
                onClick={() => void onWorkspaceChange(item.value)}
                onKeyDown={(event) => handleTabKeyDown(event, index)}
              >
                <Icon name={item.icon} size={18} />
                <span className={styles.wideLabel}>{item.label}</span>
                <span className={styles.compactLabel}>{item.compactLabel}</span>
              </button>
            );
          })}
        </div>
      </div>

      <div ref={panelRef}>
        <RouteTransition routeKey={workspace} relation="sibling" className={styles.panel}>
          {workspace === "decks" ? (
            <DeckLibraryWorkspace
              decks={decks}
              settings={settings}
              refreshDecks={refreshDecks}
              onPracticeDeck={onPracticeDeck}
              openDeckRequest={openDeckRequest}
              onOpenDeckRequestConsumed={onOpenDeckRequestConsumed}
              onManageTemplates={() => onWorkspaceChange("templates")}
              onCreateCards={() => onWorkspaceChange("create")}
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
          {workspace === "create" ? (
            <CreateCardsWorkspace
              decks={decks}
              refreshDecks={refreshDecks}
              onDirtyChange={onCardDirtyChange}
              showToast={showToast}
            />
          ) : null}
        </RouteTransition>
      </div>
    </PageScaffold>
  );
}

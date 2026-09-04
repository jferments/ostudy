import { useEffect, useRef, useState } from "react";

import type { DeckOpenRequest } from "../../App";
import { SourceCitationPanel } from "../../components/SourceCitationPanel";
import type { ShowToast } from "../../components/Toast";
import { DecksController } from "../../controllers/decksController";
import { getErrorMessage } from "../../ipc/common";
import type { DeckSummary } from "../../ipc/decks";
import { registerPlatformBackHandler } from "../../platform/backNavigation";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { ResponsiveActionGroup } from "../../ui/patterns/ResponsiveActionGroup";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { ownedClassNames, sharedClassMaps } from "../../ui/styles/classOwnership";
import ownershipStyles from "../DecksScreen.module.css";
import {
  CardBrowserWorkspace,
  type CardBrowserView,
} from "../cards/CardBrowserWorkspace";
import { useCardPracticeLauncher } from "../cards/CardPracticeContext";
import { DeckTransferDialog } from "./DeckTransferDialog";
import polishStyles from "./DeckDetailRoute.module.css";

const ownedStyleMaps = [...sharedClassMaps, ownershipStyles] as const;

interface DeckDetailRouteProps {
  decks: DeckSummary[];
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeDeck: (deckId: string) => void | Promise<void>;
  openRequest: DeckOpenRequest;
  onReturnToLibrary: () => void | Promise<void>;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

/**
 * Route-owned deck detail. It never mounts the legacy deck-card library, so a
 * deck-row click immediately renders that deck's card browser while the latest
 * complete deck summary is refreshed in the background.
 */
export function DeckDetailRoute({
  decks,
  refreshDecks,
  onPracticeDeck,
  openRequest,
  onReturnToLibrary,
  onDirtyChange,
  showToast,
}: DeckDetailRouteProps) {
  const initialDeck = decks.find((candidate) => candidate.id === openRequest.deckId) ?? null;
  const [deck, setDeck] = useState<DeckSummary | null>(initialDeck);
  const [browserView, setBrowserView] = useState<CardBrowserView>("list");
  const [isLoading, setIsLoading] = useState(initialDeck === null);
  const [showExportDialog, setShowExportDialog] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const controllerRef = useRef<DecksController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new DecksController();
  const controller = controllerRef.current;
  const confirm = useConfirm();
  const onPracticeCards = useCardPracticeLauncher();

  useEffect(() => {
    let active = true;
    setIsLoading(initialDeck === null);
    void controller
      .deck(openRequest.deckId)
      .then((loaded) => {
        if (!active) return;
        if (loaded) setDeck(loaded);
        else setErrorMessage("This deck is no longer available.");
      })
      .catch((error: unknown) => {
        if (active) setErrorMessage(getErrorMessage(error));
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
      controller.invalidate();
    };
  }, [controller, initialDeck, openRequest.deckId]);

  useEffect(() => {
    const refreshed = decks.find((candidate) => candidate.id === openRequest.deckId);
    if (refreshed) setDeck(refreshed);
  }, [decks, openRequest.deckId]);

  useEffect(() => {
    if (browserView !== "list") return;
    return registerPlatformBackHandler(() => {
      void onReturnToLibrary();
      return true;
    });
  }, [browserView, onReturnToLibrary]);

  async function reconcile(message: string): Promise<void> {
    try {
      const refreshed = await refreshDecks();
      const next = refreshed.find((candidate) => candidate.id === openRequest.deckId);
      if (next) setDeck(next);
    } catch (error: unknown) {
      showToast("info", `${message}, but the deck list could not refresh: ${getErrorMessage(error)}`);
    }
  }

  async function archive(): Promise<void> {
    if (!deck) return;
    try {
      await controller.setArchived({
        deckId: deck.id,
        revision: deck.revision,
        archived: true,
      });
      showToast("success", `Archived “${deck.name}”; all generated cards are suspended.`);
      await reconcile("The deck was archived");
      await onReturnToLibrary();
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function deleteDeck(): Promise<void> {
    if (!deck) return;
    const accepted = await confirm({
      title: `Delete “${deck.name}”?`,
      description: `This will delete ${deck.generatedCardCount} ${
        deck.generatedCardCount === 1 ? "card" : "cards"
      } and their review history after the Undo period.`,
      confirmLabel: "Delete deck",
      destructive: true,
    });
    if (!accepted) return;
    try {
      const deletion = await controller.deleteDeck(deck.id);
      showToast("success", `Deleted “${deck.name}”.`, {
        label: "Undo",
        expiresAt: deletion.undoExpiresAt,
        run: async () => {
          await controller.undo(deletion.undoToken);
          await refreshDecks();
        },
      });
      await onReturnToLibrary();
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  if (!deck && isLoading) return <LoadingState label="Opening deck…" />;
  if (!deck) {
    return (
      <InlineNotice kind="error">
        {errorMessage ?? "This deck could not be opened."}
      </InlineNotice>
    );
  }

  return (
    <div
      className={`${ownedClassNames(
        ownedStyleMaps,
        browserView === "editor" ? "deck-card-browser-editor-host" : "screen decks-screen",
      )} ${polishStyles.route}`}
    >
      {browserView === "list" ? (
        <>
          <header
            className={ownedClassNames(ownedStyleMaps, "screen-header deck-detail-header")}
          >
            <button
              type="button"
              className={ownedClassNames(ownedStyleMaps, "back-button")}
              onClick={() => void onReturnToLibrary()}
            >
              ← All decks
            </button>
            <div className={ownedClassNames(ownedStyleMaps, "deck-title-row")}>
              <h1>{deck.name}</h1>
              {deck.description ? <p>{deck.description}</p> : null}
            </div>
            <ResponsiveActionGroup
              label={`${deck.name} actions`}
              className={ownershipStyles["deck-detail-actions"]}
              actions={[
                {
                  id: "practice",
                  label: "Practice",
                  placement: "always-visible",
                  variant: "primary",
                  disabled: deck.generatedCardCount === 0 || deck.archivedAt !== null,
                  description:
                    deck.generatedCardCount === 0
                      ? "Add a flashcard before practicing this deck"
                      : `Practice all ${deck.generatedCardCount} cards in this deck`,
                  onSelect: () => void onPracticeDeck(deck.id),
                },
                {
                  id: "export",
                  label: `Export ${deck.name}`,
                  description: "Save deck to file",
                  placement: "always-visible",
                  presentation: "icon",
                  icon: "save",
                  onSelect: () => setShowExportDialog(true),
                },
                {
                  id: "delete",
                  label: `Delete ${deck.name}`,
                  description: "Delete deck",
                  placement: "always-visible",
                  presentation: "icon",
                  icon: "delete",
                  destructive: true,
                  onSelect: () => void deleteDeck(),
                },
                {
                  id: "archive",
                  label: `Archive ${deck.name}`,
                  placement: "wide-only",
                  presentation: "icon",
                  icon: "archive",
                  onSelect: () => void archive(),
                },
              ]}
            />
          </header>
          {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
        </>
      ) : null}

      <CardBrowserWorkspace
        key={`deck-card-browser:${deck.id}`}
        decks={decks}
        fixedDeckId={deck.id}
        showFilters={false}
        showPageHeader={false}
        refreshDecks={refreshDecks}
        onPracticeCards={onPracticeCards}
        onDirtyChange={onDirtyChange}
        onViewChange={setBrowserView}
        showToast={showToast}
      />

      {browserView === "list" ? (
        <footer className={polishStyles.sourceFooter}>
          <SourceCitationPanel deckId={deck.id} />
        </footer>
      ) : null}

      {showExportDialog ? (
        <DeckTransferDialog
          request={{ kind: "export", decks: [deck] }}
          refreshDecks={refreshDecks}
          onClose={() => setShowExportDialog(false)}
          showToast={showToast}
        />
      ) : null}
    </div>
  );
}

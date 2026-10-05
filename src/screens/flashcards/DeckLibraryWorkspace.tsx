import { useEffect, useRef, useState } from "react";

import type { DeckOpenRequest } from "../../App";
import type { ShowToast } from "../../components/Toast";
import type { DeckSummary } from "../../ipc/decks";
import type { AppSettings } from "../../ipc/settings";
import type { SelectedPracticeRequest } from "../../ipc/practice";
import type {
  SubjectFocusLaunch,
  TransientDeckSelectionEntry,
} from "../../navigation/flashcardsRoutes";
import { PerformanceProfiler } from "../../testing/PerformanceProfiler";
import { useConfirm } from "../../ui/overlays/overlayContext";
import {
  DeckTransferDialog,
  type DeckTransferDialogRequest,
} from "./DeckTransferDialog";
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
  subjectFocus?: SubjectFocusLaunch;
  openDeckRequest: DeckOpenRequest | null;
  onOpenDeck: (
    deckId: string,
    returnDeckSelection?: readonly TransientDeckSelectionEntry[],
  ) => void;
  onReturnToLibrary: () => void | Promise<void>;
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
  subjectFocus,
  openDeckRequest,
  onOpenDeck,
  onReturnToLibrary,
  onCardDirtyChange,
  showToast,
}: DeckLibraryWorkspaceProps) {
  const controllerSelectionRef = useRef<TransientDeckSelectionEntry[]>([]);
  const [transferRequest, setTransferRequest] =
    useState<DeckTransferDialogRequest | null>(null);
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
  useSubjectFocus(subjectFocus, controller, openDeckRequest === null);

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
    <>
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
          onImportDeck={() => setTransferRequest({ kind: "import" })}
          onExportDeck={(deck) => setTransferRequest({ kind: "export", decks: [deck] })}
          onExportSelected={(selectedDecks) =>
            setTransferRequest({ kind: "export", decks: selectedDecks })
          }
        />
      </PerformanceProfiler>
      {transferRequest ? (
        <DeckTransferDialog
          request={transferRequest}
          refreshDecks={refreshDecks}
          onImportedDeck={(outcome) => onOpenDeck(outcome.deckId)}
          onClose={() => setTransferRequest(null)}
          showToast={showToast}
        />
      ) : null}
    </>
  );
}

/**
 * Brings the subject Home asked for into view once the library has rendered:
 * its folder, expanded, at the top of the page, or the unsorted decks for No
 * subject. It jumps without animating, and each request is handled once.
 */
function useSubjectFocus(
  focus: SubjectFocusLaunch | undefined,
  controller: ReturnType<typeof useDeckLibraryController>,
  libraryVisible: boolean,
): void {
  const handledRequestId = useRef<number | null>(null);
  const latest = useRef(controller);
  latest.current = controller;
  const ready = controller.snapshot !== null;
  useEffect(() => {
    if (!focus || !ready || !libraryVisible || handledRequestId.current === focus.requestId) {
      return;
    }
    handledRequestId.current = focus.requestId;
    const { subjectId } = focus;
    if (subjectId !== null && latest.current.collapsedFolderIds.has(subjectId)) {
      latest.current.expandFolder(subjectId);
    }
    const target =
      subjectId === null
        ? document.querySelector('[data-geometry-key="destination:unsorted"]')
        : [...document.querySelectorAll<HTMLElement>("[data-folder-id]")].find(
            (element) => element.dataset.folderId === subjectId,
          );
    target?.scrollIntoView?.({ block: "start" });
  }, [focus, libraryVisible, ready]);
}

function controllerSelectionEntries(
  selection: ReturnType<typeof useDeckLibraryController>["selection"],
): TransientDeckSelectionEntry[] {
  return [...selection.entries.values()].map(({ id, revision }) => ({ id, revision }));
}

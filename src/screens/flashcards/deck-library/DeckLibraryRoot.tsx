import { useMemo, useRef } from "react";

import type { DeckSummary } from "../../../ipc/decks";
import type { SelectedPracticeRequest } from "../../../ipc/practice";
import { Icon } from "../../../ui/icons/Icon";
import { LoadingState } from "../../../ui/primitives/EmptyState";
import { InlineNotice } from "../../../ui/primitives/InlineNotice";
import { VisuallyHidden } from "../../../ui/primitives/VisuallyHidden";
import { Button } from "../../../ui/primitives/Button";
import { SelectionToolbar } from "../../../ui/patterns/SelectionToolbar";
import { DeckLibrarySheets } from "./DeckLibrarySheets";
import { DeckLibraryView } from "./DeckLibraryView";
import styles from "./DeckLibraryView.module.css";
import { LibraryDragFeedback } from "./LibraryDragFeedback";
import type { DeckLibrarySnapshot } from "./organizationModel";
import { practiceSelectionValidation } from "./practiceSetupModel";
import type { DeckLibraryController } from "./useDeckLibraryController";
import { useDeckLibraryPointerSession } from "./useDeckLibraryPointerSession";
import { useDeckLibraryReorder } from "./useDeckLibraryReorder";

interface DeckLibraryRootProps {
  controller: DeckLibraryController;
  onPracticeDeck(deckId: string): void | Promise<void>;
  decks: readonly DeckSummary[];
  practiceShuffle: boolean;
  onPracticeSelected(request: SelectedPracticeRequest): void | Promise<void>;
}

export function DeckLibraryRoot({
  controller,
  onPracticeDeck,
  decks,
  practiceShuffle,
  onPracticeSelected,
}: DeckLibraryRootProps) {
  const guidance = selectionGuidance(controller);
  const practiceValidation = practiceSelectionValidation(controller.selectionAnalysis);
  const selectionUnavailable =
    controller.selectionAnalysis.staleIds.length > 0 ||
    controller.selectionAnalysis.missingIds.length > 0;
  const allCurrentScopeSelected =
    controller.currentScopeDeckCount > 0 &&
    controller.selectedIds.size === controller.currentScopeDeckCount;
  const selectionLabel = allCurrentScopeSelected
    ? `Deselect all (${controller.currentScopeDeckCount})`
    : `Select all (${controller.currentScopeDeckCount})`;

  return (
    <div className={styles.root} aria-busy={controller.isLoading || controller.isMutating}>
      <div className={styles.toolbar} aria-label="Deck library controls">
        <div className={styles.selectionControls} role="group" aria-label="Deck selection controls">
          <Button
            size="small"
            variant="secondary"
            disabled={controller.currentScopeDeckCount === 0 || controller.isMutating}
            onClick={
              allCurrentScopeSelected ? controller.clearSelection : controller.selectCurrentScope
            }
          >
            {selectionLabel}
          </Button>
        </div>
        <div className={styles.toolbarActions}>
          <Button variant="primary" onClick={controller.openDeckCreator}>
            <Icon name="plus" size={18} /> New deck
          </Button>
        </div>
      </div>

      <SelectionToolbar
        selectedCount={controller.selectedIds.size}
        label="Selected deck actions"
        primaryAction={{
          id: "practice",
          label: "Practice",
          disabled: !practiceValidation.valid || controller.isMutating,
          onSelect: controller.openPracticeSetup,
        }}
        secondaryActions={[
          {
            id: "move",
            label: "Move",
            placement: "always-visible",
            disabled:
              controller.selectionAnalysis.activeIds.length === 0 ||
              controller.selectionAnalysis.archivedIds.length > 0 ||
              selectionUnavailable ||
              controller.isMutating,
            onSelect: controller.openSelectionMover,
          },
          ...(controller.selectionAnalysis.activeIds.length > 0
            ? [
                {
                  id: "archive",
                  label: controller.selectionAnalysis.isMixedLifecycle
                    ? "Archive active selected"
                    : "Archive selected",
                  description: "Archive selected decks",
                  placement: "always-visible" as const,
                  presentation: "icon" as const,
                  icon: "archive" as const,
                  disabled: selectionUnavailable || controller.isMutating,
                  onSelect: () => void controller.setSelectionArchived(true),
                },
              ]
            : []),
          ...(controller.selectionAnalysis.archivedIds.length > 0
            ? [
                {
                  id: "restore",
                  label: controller.selectionAnalysis.isMixedLifecycle
                    ? "Restore archived selected"
                    : "Restore selected",
                  description: "Restore selected decks",
                  placement: "always-visible" as const,
                  presentation: "icon" as const,
                  icon: "restore" as const,
                  disabled: selectionUnavailable || controller.isMutating,
                  onSelect: () => void controller.setSelectionArchived(false),
                },
              ]
            : []),
          {
            id: "delete",
            label: "Delete selected",
            description: "Delete selected decks",
            placement: "always-visible",
            presentation: "icon",
            icon: "delete",
            destructive: true,
            disabled: selectionUnavailable || controller.isMutating,
            onSelect: () => void controller.deleteSelection(),
          },
        ]}
        onClearSelection={controller.clearSelection}
      />
      {guidance ? <InlineNotice kind="info">{guidance}</InlineNotice> : null}

      <VisuallyHidden as="div" aria-live="polite">
        {controller.announcement}
      </VisuallyHidden>
      {controller.errorMessage ? (
        <InlineNotice kind="error">{controller.errorMessage}</InlineNotice>
      ) : null}
      {controller.isLoading ? <LoadingState label="Loading deck organization…" /> : null}
      {controller.snapshot ? (
        <ReadyDeckLibrary
          controller={controller}
          snapshot={controller.snapshot}
          onPracticeDeck={(deckId) => void onPracticeDeck(deckId)}
        />
      ) : null}
      <DeckLibrarySheets
        sheet={controller.sheet}
        busy={controller.isMutating}
        folders={controller.snapshot?.folders ?? []}
        decks={decks}
        practiceShuffle={practiceShuffle}
        onClose={controller.closeSheet}
        onCreateDeck={controller.createDeck}
        onSaveFolder={controller.saveFolder}
        onMoveSelection={controller.moveSelection}
        onStartPractice={onPracticeSelected}
      />
    </div>
  );
}

function selectionGuidance(controller: DeckLibraryController): string | null {
  const analysis = controller.selectionAnalysis;
  if (analysis.selectedIds.length === 0) return null;
  if (analysis.staleIds.length > 0) {
    return "One or more selected decks changed after selection. Clear and reselect them before a bulk mutation.";
  }
  if (analysis.isMixedLifecycle) {
    return "Active and archived decks are selected. Move and Practice require active decks; Archive and Restore affect only their valid lifecycle subset.";
  }
  if (analysis.archivedIds.length > 0) {
    return "Archived decks cannot move or start Practice until they are restored.";
  }
  if (analysis.emptyIds.length > 0) {
    return "This deck has no generated cards yet, so Practice is unavailable.";
  }
  return null;
}

function ReadyDeckLibrary({
  controller,
  snapshot,
  onPracticeDeck,
}: Pick<DeckLibraryRootProps, "controller" | "onPracticeDeck"> & {
  snapshot: DeckLibrarySnapshot;
}) {
  const libraryRef = useRef<HTMLDivElement | null>(null);
  const structureKey = useMemo(
    () =>
      `${snapshot.organization.revision}:${snapshot.folders
        .map((folder) => `${folder.id}:${controller.collapsedFolderIds.has(folder.id)}`)
        .join("|")}:${snapshot.activeDecks.map((deck) => deck.id).join("|")}`,
    [controller.collapsedFolderIds, snapshot],
  );
  const pointer = useDeckLibraryPointerSession(
    libraryRef,
    snapshot,
    controller.isMutating,
    structureKey,
    controller.move,
    controller.expandFolder,
  );
  const reorder = useDeckLibraryReorder(
    snapshot,
    controller.isMutating,
    controller.move,
    pointer.beginPointerSession,
  );
  return (
    <>
      <DeckLibraryView
        controller={controller}
        reorder={reorder}
        libraryRef={libraryRef}
        onPracticeDeck={onPracticeDeck}
      />
      <LibraryDragFeedback feedback={pointer.feedback} />
    </>
  );
}

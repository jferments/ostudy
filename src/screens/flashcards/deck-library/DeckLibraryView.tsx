import { memo, type RefObject } from "react";

import type { DeckSummary } from "../../../ipc/decks";
import { Icon } from "../../../ui/icons/Icon";
import { IconButton } from "../../../ui/icons/IconButton";
import { Button } from "../../../ui/primitives/Button";
import { ArchivedDecksSection } from "./ArchivedDecksSection";
import type { DeckLibraryController } from "./useDeckLibraryController";
import { DeckItem } from "./DeckItem";
import listStyles from "./DeckList.module.css";
import styles from "./DeckLibraryView.module.css";
import { FolderSection } from "./FolderSection";
import type { DeckLibraryReorderAdapter } from "./useDeckLibraryReorder";
import { UNSORTED_DATA_VALUE } from "./useDeckLibraryReorder";

interface DeckLibraryViewProps {
  controller: DeckLibraryController;
  reorder: DeckLibraryReorderAdapter;
  libraryRef: RefObject<HTMLDivElement | null>;
  onPracticeDeck(deckId: string): void | Promise<void>;
  onExportDeck(deck: DeckSummary): void;
}

export const DeckLibraryView = memo(function DeckLibraryView({
  controller,
  reorder,
  libraryRef,
  onPracticeDeck,
  onExportDeck,
}: DeckLibraryViewProps) {
  const snapshot = controller.snapshot;
  if (!snapshot) return null;

  const hasActiveDecks = snapshot.activeDecks.length > 0;
  const hasAnyDecks = hasActiveDecks || snapshot.archivedDecks.length > 0;
  const allCurrentScopeSelected =
    controller.currentScopeDeckCount > 0 &&
    controller.selectedIds.size === controller.currentScopeDeckCount;

  if (!hasAnyDecks) {
    return (
      <div ref={libraryRef} className={styles.library} data-deck-library-pointer-surface="true">
        <section className={styles.emptyLibrary} aria-labelledby="empty-deck-library-heading">
          <span className={styles.emptyLibraryIcon} aria-hidden="true">
            <Icon name="cards" size={30} />
          </span>
          <div className={styles.emptyLibraryCopy}>
            <h2 id="empty-deck-library-heading">No decks have been created yet.</h2>
            <p>
              Create a new deck of flashcards here, or use Import deck from file above to add an
              existing flashcard set.
            </p>
          </div>
          <Button
            variant="primary"
            aria-label="Create a new deck"
            onClick={controller.openDeckCreator}
          >
            <Icon name="plus" size={18} /> New deck
          </Button>
        </section>
      </div>
    );
  }

  return (
    <div ref={libraryRef} className={styles.library} data-deck-library-pointer-surface="true">
      {hasActiveDecks ? (
        <section className={styles.folderArea} aria-labelledby="folders-heading">
          <div className={styles.sectionHeading}>
            <div className={styles.sectionTitle}>
              <div>
                <h2 id="folders-heading">Folders</h2>
                <p>
                  {snapshot.folders.length} {snapshot.folders.length === 1 ? "folder" : "folders"}
                </p>
              </div>
              <IconButton
                icon="plus"
                iconSize={16}
                label="Create a folder"
                tooltip="New folder"
                disabled={controller.isMutating}
                onClick={controller.openFolderCreator}
              />
            </div>
            <div className={styles.sectionActions}>
              {snapshot.folders.length > 0 ? (
                <Button
                  size="small"
                  variant="ghost"
                  disabled={controller.isMutating}
                  onClick={() => {
                    if (controller.allFoldersCollapsed) {
                      for (const folder of snapshot.folders) controller.expandFolder(folder.id);
                    } else {
                      controller.collapseAllFolders();
                    }
                  }}
                >
                  {controller.allFoldersCollapsed ? "Expand all" : "Collapse all"}
                </Button>
              ) : null}
              {controller.currentScopeDeckCount > 0 ? (
                <Button
                  size="small"
                  variant="ghost"
                  disabled={controller.isMutating}
                  onClick={
                    allCurrentScopeSelected
                      ? controller.clearSelection
                      : controller.selectCurrentScope
                  }
                >
                  {allCurrentScopeSelected
                    ? `Deselect all (${controller.currentScopeDeckCount})`
                    : `Select all (${controller.currentScopeDeckCount})`}
                </Button>
              ) : null}
            </div>
          </div>
          {snapshot.folders.length > 0 ? (
            <div className={styles.folderList}>
              {snapshot.folders.map((folder, folderIndex) => (
                <FolderSection
                  key={folder.id}
                  folder={folder}
                  decks={snapshot.decksByFolder.get(folder.id) ?? []}
                  folderIndex={folderIndex}
                  folderCount={snapshot.folders.length}
                  disabled={controller.isMutating}
                  collapsed={controller.collapsedFolderIds.has(folder.id)}
                  reorder={reorder}
                  onToggleCollapsed={() => controller.toggleFolder(folder.id)}
                  onOpenDeck={controller.openDeck}
                  onPracticeDeck={(deckId) => void onPracticeDeck(deckId)}
                  onExportDeck={onExportDeck}
                  onDeleteDeck={(deckId) => void controller.removeDeck(deckId)}
                  onRename={() => controller.openFolderRenamer(folder.id)}
                  onDelete={() => void controller.removeFolder(folder.id)}
                  selectedIds={controller.selectedIds}
                  onToggleDeckSelected={controller.toggleDeckSelected}
                />
              ))}
            </div>
          ) : null}
        </section>
      ) : null}

      {snapshot.unsortedDecks.length > 0 ? (
        <section
          className={styles.unsortedArea}
          aria-labelledby="unsorted-heading"
          data-library-folder-surface="true"
          data-geometry-key="destination:unsorted"
          data-folder-id={UNSORTED_DATA_VALUE}
          data-destination-name="Unsorted decks"
          data-collapsed="false"
        >
          <div className={styles.sectionHeading}>
            <div>
              <h2 id="unsorted-heading">Unsorted decks</h2>
              <p>
                {snapshot.unsortedDecks.length}{" "}
                {snapshot.unsortedDecks.length === 1 ? "deck" : "decks"}
              </p>
            </div>
          </div>
          <div className={listStyles.list} data-library-deck-list="true">
            {snapshot.unsortedDecks.map((deck, index) => (
              <UnsortedDeckItem
                key={deck.id}
                deck={deck}
                index={index}
                count={snapshot.unsortedDecks.length}
                controller={controller}
                reorder={reorder}
                onPracticeDeck={onPracticeDeck}
                onExportDeck={onExportDeck}
              />
            ))}
          </div>
        </section>
      ) : null}

      <ArchivedDecksSection
        decks={snapshot.archivedDecks}
        expanded={controller.showArchived}
        disabled={controller.isMutating}
        reorder={reorder}
        onToggle={controller.toggleArchived}
        onOpenDeck={controller.openDeck}
        onExportDeck={onExportDeck}
        onDeleteDeck={(deckId) => void controller.removeDeck(deckId)}
        selectedIds={controller.selectedIds}
        onToggleDeckSelected={controller.toggleDeckSelected}
      />
    </div>
  );
});

function UnsortedDeckItem({
  deck,
  index,
  count,
  controller,
  reorder,
  onPracticeDeck,
  onExportDeck,
}: {
  deck: DeckSummary;
  index: number;
  count: number;
  controller: DeckLibraryController;
  reorder: DeckLibraryReorderAdapter;
  onPracticeDeck(deckId: string): void | Promise<void>;
  onExportDeck(deck: DeckSummary): void;
}) {
  return (
    <DeckItem
      deck={deck}
      folderId={null}
      index={index}
      count={count}
      disabled={controller.isMutating}
      selected={controller.selectedIds.has(deck.id)}
      reorder={reorder}
      onOpen={() => controller.openDeck(deck.id)}
      onPractice={() => void onPracticeDeck(deck.id)}
      onExport={() => onExportDeck(deck)}
      onDelete={() => void controller.removeDeck(deck.id)}
      onToggleSelected={() => controller.toggleDeckSelected(deck)}
    />
  );
}

import { memo, type RefObject } from "react";

import type { DeckSummary } from "../../../ipc/decks";
import { HelpTrigger } from "../../../ui/help/HelpProvider";
import { Icon } from "../../../ui/icons/Icon";
import { Button } from "../../../ui/primitives/Button";
import { Surface } from "../../../ui/primitives/Surface";
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
}

export const DeckLibraryView = memo(function DeckLibraryView({
  controller,
  reorder,
  libraryRef,
  onPracticeDeck,
}: DeckLibraryViewProps) {
  const snapshot = controller.snapshot;
  if (!snapshot) return null;

  return (
    <div ref={libraryRef} className={styles.library} data-deck-library-pointer-surface="true">
      <section className={styles.folderArea} aria-labelledby="folders-heading">
        <div className={styles.sectionHeading}>
          <div>
            <h2 id="folders-heading">Folders</h2>
            <p>
              {snapshot.folders.length} {snapshot.folders.length === 1 ? "folder" : "folders"}
            </p>
          </div>
          <div className={styles.sectionActions}>
            <Button size="small" onClick={controller.openFolderCreator}>
              <Icon name="folder" size={17} /> New folder
            </Button>
            <Button
              size="small"
              variant="ghost"
              disabled={snapshot.folders.length === 0 || controller.allFoldersCollapsed}
              onClick={controller.collapseAllFolders}
            >
              Collapse all
            </Button>
            <HelpTrigger topicId="flashcards.folders" label="Help with folders" />
          </div>
        </div>
        {snapshot.folders.length === 0 ? (
          <Surface level={1} material="soft" className={styles.emptyFolderHint}>
            <Icon name="folder" size={24} />
            <span>No folders yet.</span>
          </Surface>
        ) : (
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
                onDeleteDeck={(deckId) => void controller.removeDeck(deckId)}
                onRename={() => controller.openFolderRenamer(folder.id)}
                onDelete={() => void controller.removeFolder(folder.id)}
                selectedIds={controller.selectedIds}
                onToggleDeckSelected={controller.toggleDeckSelected}
              />
            ))}
          </div>
        )}
      </section>

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
        {snapshot.unsortedDecks.length === 0 ? (
          <Surface
            level={1}
            material="soft"
            className={styles.emptyFolderHint}
            data-library-deck-list="true"
          >
            <Icon name="check" size={24} />
            <span>All active decks are in folders.</span>
          </Surface>
        ) : (
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
              />
            ))}
          </div>
        )}
      </section>

      <ArchivedDecksSection
        decks={snapshot.archivedDecks}
        expanded={controller.showArchived}
        disabled={controller.isMutating}
        reorder={reorder}
        onToggle={controller.toggleArchived}
        onOpenDeck={controller.openDeck}
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
}: {
  deck: DeckSummary;
  index: number;
  count: number;
  controller: DeckLibraryController;
  reorder: DeckLibraryReorderAdapter;
  onPracticeDeck(deckId: string): void | Promise<void>;
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
      onDelete={() => void controller.removeDeck(deck.id)}
      onToggleSelected={() => controller.toggleDeckSelected(deck)}
    />
  );
}

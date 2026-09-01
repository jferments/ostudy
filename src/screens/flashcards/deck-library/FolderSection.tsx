import { memo } from "react";

import type { DeckSummary } from "../../../ipc/decks";
import type { DeckFolder } from "../../../ipc/deckFolders";
import { Artwork } from "../../../ui/artwork/Artwork";
import { Icon } from "../../../ui/icons/Icon";
import { Surface } from "../../../ui/primitives/Surface";
import { VisuallyHidden } from "../../../ui/primitives/VisuallyHidden";
import { OverflowMenu } from "../../../ui/overlays/OverflowMenu";
import { useLayoutMode } from "../../../ui/layout/LayoutModeProvider";
import { DeckItem } from "./DeckItem";
import listStyles from "./DeckList.module.css";
import styles from "./FolderSection.module.css";
import type { DeckLibraryReorderAdapter } from "./useDeckLibraryReorder";

interface FolderSectionProps {
  folder: DeckFolder;
  decks: DeckSummary[];
  folderIndex: number;
  folderCount: number;
  disabled: boolean;
  collapsed: boolean;
  reorder: DeckLibraryReorderAdapter;
  onToggleCollapsed(): void;
  onOpenDeck(deckId: string): void;
  onPracticeDeck(deckId: string): void;
  onDeleteDeck(deckId: string): void;
  onRename(): void;
  onDelete(): void;
  selectedIds: ReadonlySet<string>;
  onToggleDeckSelected(deck: DeckSummary): void;
}

export const FolderSection = memo(function FolderSection({
  folder,
  decks,
  folderIndex,
  folderCount,
  disabled,
  collapsed,
  reorder,
  onToggleCollapsed,
  onOpenDeck,
  onPracticeDeck,
  onDeleteDeck,
  onRename,
  onDelete,
  selectedIds,
  onToggleDeckSelected,
}: FolderSectionProps) {
  const { layoutMode } = useLayoutMode();
  const due = decks.reduce((sum, deck) => sum + deck.studyableDueCount, 0);
  const contentId = `deck-folder-${folder.id}`;
  return (
    <Surface
      as="section"
      level={2}
      className={styles.folder}
      data-library-row="folder"
      data-library-folder-surface="true"
      data-library-pointer-drag-source="true"
      data-geometry-key={`folder:${folder.id}`}
      data-item-id={folder.id}
      data-item-index={folderIndex}
      data-item-name={folder.name}
      data-folder-id={folder.id}
      data-folder-index={folderIndex}
      data-destination-name={folder.name}
      data-collapsed={collapsed ? "true" : "false"}
      onPointerDown={(event) =>
        reorder.beginPointerDrag(
          event,
          { kind: "folder", id: folder.id },
          `${folder.name} folder`,
        )
      }
    >
      <header className={styles.header}>
        <button
          type="button"
          className={styles.keyboardReorder}
          disabled={disabled}
          aria-label={`Reorder ${folder.name} folder. Use Up and Down arrow keys.`}
          aria-keyshortcuts="ArrowUp ArrowDown Home End"
          title="Reorder folder with keyboard"
          onKeyDown={(event) =>
            reorder.moveFolderWithKeyboard(event, folder.id, folderIndex)
          }
        >
          <Icon name="move" size={16} />
        </button>
        <button
          type="button"
          className={styles.toggle}
          aria-expanded={!collapsed}
          aria-controls={contentId}
          aria-label={`${collapsed ? "Expand" : "Collapse"} ${folder.name} folder`}
          title={collapsed ? "Expand folder" : "Collapse folder"}
          onClick={onToggleCollapsed}
        >
          <span className={styles.icon}>
            <Icon name={collapsed ? "folder" : "folder-open"} size={22} />
          </span>
          <span className={styles.title}>
            <h3>{folder.name}</h3>
            <span>
              {decks.length} {decks.length === 1 ? "deck" : "decks"} · {due} ready
            </span>
          </span>
          <span className={styles.chevron} aria-hidden="true">
            <Icon name={collapsed ? "chevron-right" : "chevron-down"} size={18} />
          </span>
        </button>
        <div className={styles.actions}>
          {layoutMode === "compact" ? (
            <OverflowMenu
              label={`${folder.name} folder actions`}
              disabled={disabled}
              items={[
                { id: "rename", label: `Rename ${folder.name} folder`, onSelect: onRename },
                {
                  id: "delete",
                  label: `Delete ${folder.name} folder`,
                  destructive: true,
                  onSelect: onDelete,
                },
              ]}
            />
          ) : (
            <>
              <button type="button" disabled={disabled} onClick={onRename}>
                Rename
              </button>
              <button
                type="button"
                disabled={disabled}
                className={styles.danger}
                onClick={onDelete}
              >
                Delete
              </button>
            </>
          )}
        </div>
      </header>
      {!collapsed ? (
        decks.length === 0 ? (
          <div id={contentId} className={styles.empty}>
            <Artwork slot="empty.folder" />
            <p>Drag a deck here.</p>
          </div>
        ) : (
          <div id={contentId} className={listStyles.list}>
            {decks.map((deck, index) => (
              <DeckItem
                key={deck.id}
                deck={deck}
                folderId={folder.id}
                index={index}
                count={decks.length}
                disabled={disabled}
                selected={selectedIds.has(deck.id)}
                reorder={reorder}
                onOpen={() => onOpenDeck(deck.id)}
                onPractice={() => onPracticeDeck(deck.id)}
                onDelete={() => onDeleteDeck(deck.id)}
                onToggleSelected={() => onToggleDeckSelected(deck)}
              />
            ))}
          </div>
        )
      ) : null}
      <VisuallyHidden>
        Folder {folderIndex + 1} of {folderCount}.
      </VisuallyHidden>
    </Surface>
  );
});

import { memo } from "react";

import type { DeckSummary } from "../../../ipc/decks";
import { Icon } from "../../../ui/icons/Icon";
import { IconButton } from "../../../ui/icons/IconButton";
import { ResponsiveActionGroup } from "../../../ui/patterns/ResponsiveActionGroup";
import { VisuallyHidden } from "../../../ui/primitives/VisuallyHidden";
import styles from "./DeckItem.module.css";
import type { DeckLibraryReorderAdapter } from "./useDeckLibraryReorder";
import { UNSORTED_DATA_VALUE } from "./useDeckLibraryReorder";

interface DeckItemProps {
  deck: DeckSummary;
  folderId: string | null;
  index: number;
  count: number;
  disabled: boolean;
  archived?: boolean;
  selected: boolean;
  reorder: DeckLibraryReorderAdapter;
  onOpen(): void;
  onPractice(): void;
  onExport(): void;
  onDelete(): void;
  onToggleSelected(): void;
}

export const DeckItem = memo(function DeckItem({
  deck,
  folderId,
  index,
  count,
  disabled,
  archived = false,
  selected,
  reorder,
  onOpen,
  onPractice,
  onExport,
  onDelete,
  onToggleSelected,
}: DeckItemProps) {
  return (
    <article
      className={styles.item}
      data-archived={archived ? "true" : undefined}
      data-selected={selected || undefined}
      data-library-row={archived ? undefined : "deck"}
      data-geometry-key={archived ? undefined : `deck:${deck.id}`}
      data-item-id={archived ? undefined : deck.id}
      data-item-index={archived ? undefined : index}
      data-item-name={archived ? undefined : deck.name}
      data-folder-id={folderId ?? UNSORTED_DATA_VALUE}
    >
      <label className={styles.selection} data-library-no-pointer-drag="true">
        <input
          type="checkbox"
          aria-label={`Select ${deck.name}`}
          checked={selected}
          disabled={disabled}
          onChange={onToggleSelected}
        />
      </label>
      <div
        className={styles.main}
        role="button"
        tabIndex={0}
        aria-label={`Open ${deck.name}`}
        aria-keyshortcuts={
          archived
            ? "Enter Space"
            : "Enter Space ArrowUp ArrowDown Home End ArrowLeft ArrowRight"
        }
        data-library-pointer-drag-source={archived ? undefined : "true"}
        onClick={onOpen}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onOpen();
            return;
          }
          if (!archived) {
            reorder.moveDeckWithKeyboard(event, deck.id, folderId, index, count);
          }
        }}
        onPointerDown={
          archived
            ? undefined
            : (event) =>
                reorder.beginPointerDrag(
                  event,
                  { kind: "deck", id: deck.id },
                  deck.name,
                )
        }
      >
        <span className={styles.mark}>
          <Icon name="cards" size={20} />
        </span>
        <span className={styles.copy}>
          <strong>{deck.name}</strong>
          <small>
            {deck.generatedCardCount} cards · {deck.studyableDueCount} ready
          </small>
        </span>
      </div>
      <div className={styles.actions} data-library-no-pointer-drag="true">
        {!archived ? (
          <ResponsiveActionGroup
            label={`${deck.name} actions`}
            overflowLabel={`${deck.name} actions`}
            actions={[
              {
                id: "practice",
                label: "Practice",
                placement: "always-visible",
                variant: "primary",
                disabled: disabled || deck.generatedCardCount === 0,
                onSelect: onPractice,
              },
              {
                id: "export",
                label: `Export ${deck.name}`,
                description: "Save deck to file",
                placement: "always-visible",
                presentation: "icon",
                icon: "save",
                disabled,
                onSelect: onExport,
              },
              {
                id: "delete",
                label: `Delete ${deck.name}`,
                placement: "always-visible",
                presentation: "icon",
                icon: "delete",
                destructive: true,
                disabled,
                onSelect: onDelete,
              },
            ]}
          />
        ) : (
          <div className={styles.archivedActions}>
            <IconButton
              icon="save"
              label={`Export ${deck.name}`}
              tooltip="Save deck to file"
              disabled={disabled}
              onClick={onExport}
            />
            <IconButton
              icon="delete"
              label={`Delete ${deck.name}`}
              tone="danger"
              disabled={disabled}
              onClick={onDelete}
            />
          </div>
        )}
      </div>
      <VisuallyHidden>
        Deck {index + 1} of {count}.
        {!archived ? (
          <>
            <button
              type="button"
              onClick={() =>
                reorder.moveDeckWithFallback(deck.id, folderId, Math.max(0, index - 1))
              }
            >
              Move earlier
            </button>
            <button
              type="button"
              onClick={() =>
                reorder.moveDeckWithFallback(deck.id, folderId, Math.min(count - 1, index + 1))
              }
            >
              Move later
            </button>
          </>
        ) : null}
      </VisuallyHidden>
    </article>
  );
});

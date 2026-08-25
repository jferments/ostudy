import type { DeckSummary } from "../../../ipc/decks";
import { Icon } from "../../../ui/icons/Icon";
import { Button } from "../../../ui/primitives/Button";
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
  reorder: DeckLibraryReorderAdapter;
  onOpen(): void;
  onPractice(): void;
  onDelete(): void;
}

export function DeckItem({
  deck,
  folderId,
  index,
  count,
  disabled,
  archived = false,
  reorder,
  onOpen,
  onPractice,
  onDelete,
}: DeckItemProps) {
  return (
    <article
      className={styles.item}
      data-archived={archived || undefined}
      data-library-drop={archived ? undefined : "deck"}
      data-folder-id={folderId ?? UNSORTED_DATA_VALUE}
      data-target-index={index}
      onDragOver={archived ? undefined : (event) => event.preventDefault()}
      onDrop={archived ? undefined : (event) => reorder.dropDeck(event, folderId, index)}
    >
      {!archived ? (
        <span
          className={styles.dragHandle}
          draggable={!disabled}
          role="button"
          tabIndex={0}
          aria-label={`Reorder ${deck.name}. Use Up and Down within this group or Left and Right between folders.`}
          aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight Home End"
          onDragStart={(event) => reorder.startDeckDrag(event, deck.id)}
          onKeyDown={(event) =>
            reorder.moveDeckWithKeyboard(event, deck.id, folderId, index, count)
          }
          onPointerDown={(event) =>
            reorder.beginPointerDrag(event, { kind: "deck", id: deck.id })
          }
          onPointerMove={reorder.updatePointerDrag}
          onPointerUp={reorder.finishPointerDrag}
          onPointerCancel={reorder.cancelPointerDrag}
          title="Drag to reorder or move this deck"
        >
          <Icon name="move" size={16} />
        </span>
      ) : null}
      <button type="button" className={styles.main} onClick={onOpen}>
        <span className={styles.mark}>
          <Icon name="cards" size={20} />
        </span>
        <span className={styles.copy}>
          <strong>{deck.name}</strong>
          <small>
            {deck.generatedCardCount} cards · {deck.studyableDueCount} ready
          </small>
        </span>
      </button>
      <div className={styles.actions}>
        {!archived ? (
          <Button
            size="small"
            className={styles.practice}
            disabled={disabled || deck.generatedCardCount === 0}
            onClick={onPractice}
          >
            Practice
          </Button>
        ) : null}
        <button
          type="button"
          className={styles.delete}
          aria-label={`Delete ${deck.name}`}
          title={`Delete ${deck.name}`}
          disabled={disabled}
          onClick={onDelete}
        >
          <Icon name="delete" size={18} />
        </button>
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
}

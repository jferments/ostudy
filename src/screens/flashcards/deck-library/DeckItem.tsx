import type { DeckSummary } from "../../../ipc/decks";
import { Icon } from "../../../ui/icons/Icon";
import { IconButton } from "../../../ui/icons/IconButton";
import { useLayoutMode } from "../../../ui/layout/LayoutModeProvider";
import { ResponsiveActionGroup } from "../../../ui/patterns/ResponsiveActionGroup";
import { OverflowMenu } from "../../../ui/overlays/OverflowMenu";
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
  onDelete(): void;
  onToggleSelected(): void;
}

export function DeckItem({
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
  onDelete,
  onToggleSelected,
}: DeckItemProps) {
  const { layoutMode } = useLayoutMode();
  return (
    <article
      className={styles.item}
      data-archived={archived || undefined}
      data-selected={selected || undefined}
      data-library-drop={archived ? undefined : "deck"}
      data-folder-id={folderId ?? UNSORTED_DATA_VALUE}
      data-target-index={index}
      onDragOver={archived ? undefined : (event) => event.preventDefault()}
      onDrop={archived ? undefined : (event) => reorder.dropDeck(event, folderId, index)}
    >
      <label className={styles.selection}>
        <input
          type="checkbox"
          aria-label={`Select ${deck.name}`}
          checked={selected}
          disabled={disabled}
          onChange={onToggleSelected}
        />
      </label>
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
                id: "delete",
                label: `Delete ${deck.name}`,
                placement: "wide-and-medium",
                presentation: "icon",
                icon: "delete",
                destructive: true,
                disabled,
                onSelect: onDelete,
              },
            ]}
          />
        ) : layoutMode === "compact" ? (
          <OverflowMenu
            label={`${deck.name} actions`}
            disabled={disabled}
            items={[
              {
                id: "delete",
                label: `Delete ${deck.name}`,
                destructive: true,
                onSelect: onDelete,
              },
            ]}
          />
        ) : (
          <IconButton
            icon="delete"
            label={`Delete ${deck.name}`}
            tone="danger"
            disabled={disabled}
            onClick={onDelete}
          />
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
}

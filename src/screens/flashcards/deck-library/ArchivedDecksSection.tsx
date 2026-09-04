import type { DeckSummary } from "../../../ipc/decks";
import { Icon } from "../../../ui/icons/Icon";
import styles from "./ArchivedDecksSection.module.css";
import { DeckItem } from "./DeckItem";
import listStyles from "./DeckList.module.css";
import type { DeckLibraryReorderAdapter } from "./useDeckLibraryReorder";

interface ArchivedDecksSectionProps {
  decks: DeckSummary[];
  expanded: boolean;
  disabled: boolean;
  reorder: DeckLibraryReorderAdapter;
  onToggle(): void;
  onOpenDeck(deckId: string): void;
  onExportDeck(deck: DeckSummary): void;
  onDeleteDeck(deckId: string): void;
  selectedIds: ReadonlySet<string>;
  onToggleDeckSelected(deck: DeckSummary): void;
}

export function ArchivedDecksSection({
  decks,
  expanded,
  disabled,
  reorder,
  onToggle,
  onOpenDeck,
  onExportDeck,
  onDeleteDeck,
  selectedIds,
  onToggleDeckSelected,
}: ArchivedDecksSectionProps) {
  if (decks.length === 0) return null;
  return (
    <section className={styles.root}>
      <button
        type="button"
        className={styles.toggle}
        aria-expanded={expanded}
        onClick={onToggle}
      >
        <span>
          <strong>Archived decks</strong>
          <small>{decks.length} stored with cards suspended</small>
        </span>
        <Icon name={expanded ? "chevron-down" : "chevron-right"} size={18} />
      </button>
      {expanded ? (
        <div className={listStyles.list}>
          {decks.map((deck, index) => (
            <DeckItem
              key={deck.id}
              deck={deck}
              folderId={null}
              index={index}
              count={decks.length}
              disabled={disabled}
              selected={selectedIds.has(deck.id)}
              archived
              reorder={reorder}
              onOpen={() => onOpenDeck(deck.id)}
              onPractice={() => undefined}
              onExport={() => onExportDeck(deck)}
              onDelete={() => onDeleteDeck(deck.id)}
              onToggleSelected={() => onToggleDeckSelected(deck)}
            />
          ))}
        </div>
      ) : null}
    </section>
  );
}

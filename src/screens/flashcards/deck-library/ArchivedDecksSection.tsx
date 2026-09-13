import listGeometry from "./DeckList.module.css";
import type { DeckSummary } from "../../../ipc/decks";
import { Icon } from "../../../ui/icons/Icon";
import { Button } from "../../../ui/primitives/Button";
import { DeckItem } from "./DeckItem";
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
    <section className="tw:grid tw:min-w-0 tw:gap-3">
      <Button variant="ghost"
        type="button"
        className="tw:w-full tw:justify-between tw:py-4 tw:text-start tw:[&>span]:flex tw:[&>span]:w-full tw:[&>span]:items-center tw:[&>span]:justify-between tw:[&_strong]:block tw:[&_strong]:text-sm tw:[&_small]:block tw:[&_small]:text-xs tw:[&_small]:font-normal tw:[&_small]:text-muted-foreground"
        aria-expanded={expanded}
        onClick={onToggle}
      >
        <span>
          <strong>Archived decks</strong>
          <small>{decks.length} stored with cards suspended</small>
        </span>
        <Icon name={expanded ? "chevron-down" : "chevron-right"} size={18} />
      </Button>
      {expanded ? (
        <div className={listGeometry.list}>
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

import { useMemo, useState, type ReactNode } from "react";

import type { TagSummary } from "../../../ipc/cards";
import type { DeckSummary } from "../../../ipc/decks";
import type { LibrarySubjectScope } from "../../../navigation/appDestinations";
import { Icon, type IconName } from "../../../ui/icons/Icon";
import { cn } from "../../../ui/internal/cn";
import { OverflowMenu, type OverflowMenuItem } from "../../../ui/overlays/OverflowMenu";
import { Button } from "../../../ui/primitives/Button";
import { ItemTrigger } from "../../../ui/primitives/Item";
import { subjectColorIndexes, subjectInitials } from "../../home/homeModel";
import { SubjectBadge } from "../../home/SubjectBadge";
import {
  archivedDecks,
  decksInSubject,
  sameScope,
  subjectFilterOf,
  subjectNodes,
  subjectOfDeck,
  subjectRootScope,
  type WorkbenchLibrary,
  type WorkbenchScope,
} from "./workbenchModel";

/** Tags shown before "Show all tags". */
const TAG_PREVIEW = 8;

export interface WorkbenchTreeActions {
  onSelectScope: (scope: WorkbenchScope) => void;
  onNewDeck: (subjectId: string | null) => void;
  /** A phone's Import deck, beside New deck; wider screens show it with Library's tabs. */
  onImportDeck?: () => void;
  deckMenu: (deck: DeckSummary) => OverflowMenuItem[];
  onCheckThese: () => void;
}

interface WorkbenchTreeProps extends WorkbenchTreeActions {
  library: WorkbenchLibrary;
  /** The chosen subject's tags, or every tag under Everything. */
  tags: TagSummary[];
  scope: WorkbenchScope;
  checkTheseCount: number;
  /** Phones show the tree as its own page with larger rows. */
  compact: boolean;
  /** Library's chosen subject: the tree lists its decks and tags. */
  subject: LibrarySubjectScope;
}

/**
 * The Workbench's left side for Library's chosen subject: all of its cards,
 * its decks and its tags. Everything lists every deck, each with its
 * subject's badge. Choosing an entry scopes the card list; each deck carries
 * its own actions menu.
 */
export function WorkbenchTree({ library, tags, scope, checkTheseCount, compact, subject, ...actions }: WorkbenchTreeProps) {
  const subjectId = subjectFilterOf(subject);
  const colors = useMemo(() => subjectColorIndexes(library.subjects), [library.subjects]);
  const decks = useMemo(
    () =>
      subjectId === undefined
        ? subjectNodes(library).flatMap((node) => node.decks.map((deck) => ({ deck, subjectId: node.subjectId, name: node.name })))
        : decksInSubject(library, subjectId).map((deck) => ({ deck, subjectId, name: "" })),
    [library, subjectId],
  );
  const archived = useMemo(
    () => archivedDecks(library).filter((deck) => subjectId === undefined || subjectOfDeck(library, deck.id) === subjectId),
    [library, subjectId],
  );
  const totalCards = decks.reduce((sum, entry) => sum + entry.deck.generatedCardCount, 0);
  const root = subjectRootScope(subject);
  const rootLabel =
    subject.kind === "all"
      ? "All cards"
      : subject.kind === "none"
        ? "Unsorted cards"
        : `All of ${library.subjects.find((candidate) => candidate.id === subject.subjectId)?.name ?? "this subject"}`;
  const [showArchived, setShowArchived] = useState(false);
  const [showAllTags, setShowAllTags] = useState(false);
  const sortedTags = useMemo(
    () => [...tags].sort((left, right) => right.sourceNoteCount - left.sourceNoteCount || left.name.localeCompare(right.name)),
    [tags],
  );
  const visibleTags = showAllTags ? sortedTags : sortedTags.slice(0, TAG_PREVIEW);

  return (
    <nav aria-label="Library" data-tour-anchor="library.workbench" className="tw:grid tw:min-w-0 tw:content-start tw:gap-5 tw:pb-4">
      <div className="tw:grid tw:min-w-0">
        <TreeEntry
          icon="library"
          label={rootLabel}
          count={totalCards}
          current={sameScope(scope, root)}
          compact={compact}
          onSelect={() => actions.onSelectScope(root)}
          testId="workbench-root"
        />
      </div>

      <TreeSection title="Decks">
        {checkTheseCount > 0 ? (
          <Button
            variant="secondary"
            size="small"
            className="tw:mb-1 tw:justify-start"
            onClick={actions.onCheckThese}
          >
            <Icon name="information" size={16} />
            Check these ({checkTheseCount.toLocaleString()})
          </Button>
        ) : null}
        {decks.map(({ deck, subjectId: deckSubjectId, name }) => (
          <div key={deck.id} className="tw:flex tw:min-w-0 tw:items-center">
            <TreeEntry
              icon="cards"
              leading={
                subjectId === undefined ? (
                  <SubjectBadge
                    subject={{
                      subjectId: deckSubjectId,
                      initials: subjectInitials(name),
                      colorIndex: deckSubjectId === null ? null : (colors.get(deckSubjectId) ?? null),
                    }}
                    size="small"
                    className="tw:size-6 tw:text-caption"
                  />
                ) : undefined
              }
              label={deck.name}
              count={deck.generatedCardCount}
              current={sameScope(scope, { kind: "deck", deckId: deck.id })}
              compact={compact}
              onSelect={() => actions.onSelectScope({ kind: "deck", deckId: deck.id })}
              testId="workbench-deck"
            />
            <OverflowMenu label={`${deck.name} actions`} items={actions.deckMenu(deck)} className="tw:shrink-0" />
          </div>
        ))}
        <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-x-2">
          <Button
            variant="text"
            size="small"
            data-testid="workbench-new-deck"
            className="tw:justify-start tw:px-2"
            onClick={() => actions.onNewDeck(subjectId ?? null)}
          >
            <Icon name="plus" size={16} /> New deck
          </Button>
          {actions.onImportDeck ? (
            <Button variant="text" size="small" className="tw:justify-start tw:px-2" onClick={actions.onImportDeck}>
              <Icon name="import" size={16} /> Import deck
            </Button>
          ) : null}
        </div>
        {archived.length > 0 ? (
          <div className="tw:grid tw:min-w-0">
            <Button variant="ghost" size="small" aria-expanded={showArchived} className="tw:justify-start tw:text-muted-foreground" onClick={() => setShowArchived((value) => !value)}>
              <Icon name="archive" size={16} />
              {showArchived ? "Hide archived decks" : `Archived decks (${archived.length.toLocaleString()})`}
            </Button>
            {showArchived ? archived.map((deck) => (
              <div key={deck.id} className="tw:ml-6 tw:flex tw:min-w-0 tw:items-center">
                <TreeEntry
                  icon="archive"
                  label={deck.name}
                  muted
                  current={sameScope(scope, { kind: "deck", deckId: deck.id })}
                  compact={compact}
                  onSelect={() => actions.onSelectScope({ kind: "deck", deckId: deck.id })}
                />
                <OverflowMenu label={`${deck.name} actions`} items={actions.deckMenu(deck)} className="tw:shrink-0" />
              </div>
            )) : null}
          </div>
        ) : null}
      </TreeSection>

      {sortedTags.length > 0 ? (
        <TreeSection title="Tags">
          {visibleTags.map((tag) => (
            <TreeEntry
              key={tag.id}
              icon="filter"
              label={tag.name}
              count={tag.sourceNoteCount}
              current={sameScope(scope, { kind: "tag", tagId: tag.id, subjectId })}
              compact={compact}
              onSelect={() => actions.onSelectScope({ kind: "tag", tagId: tag.id, subjectId })}
            />
          ))}
          {sortedTags.length > TAG_PREVIEW ? (
            <Button variant="text" size="small" className="tw:justify-start tw:px-2" onClick={() => setShowAllTags((value) => !value)}>
              {showAllTags ? "Show fewer tags" : `Show all ${sortedTags.length.toLocaleString()} tags`}
            </Button>
          ) : null}
        </TreeSection>
      ) : null}
    </nav>
  );
}

function TreeSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="tw:grid tw:min-w-0 tw:gap-0.5">
      <div className="tw:flex tw:min-h-8 tw:items-center tw:justify-between tw:gap-2 tw:px-2">
        <h2 className="tw:m-0 tw:text-caption tw:font-semibold tw:tracking-wide tw:text-muted-foreground tw:uppercase">{title}</h2>
      </div>
      {children}
    </section>
  );
}

interface TreeEntryProps {
  label: string;
  icon?: IconName;
  leading?: ReactNode;
  count?: number;
  current?: boolean;
  muted?: boolean;
  compact: boolean;
  onSelect: () => void;
  testId?: string;
}

function TreeEntry({ label, icon, leading, count, current = false, muted = false, compact, onSelect, testId }: TreeEntryProps) {
  return (
    <ItemTrigger
      data-testid={testId}
      aria-current={current ? "true" : undefined}
      onClick={onSelect}
      className={cn(
        "tw:flex tw:min-w-0 tw:flex-1 tw:cursor-pointer tw:items-center tw:gap-2 tw:rounded-md tw:border-0 tw:bg-transparent tw:px-2 tw:text-left tw:text-foreground tw:transition-colors tw:hover:bg-muted tw:reduce-motion:transition-none",
        compact ? "tw:min-h-11 tw:text-body" : "tw:min-h-8 tw:text-small tw:coarse:min-h-11",
        current && "tw:bg-accent tw:font-semibold tw:text-accent-foreground tw:hover:bg-accent",
        muted && !current && "tw:text-muted-foreground",
      )}
    >
      {leading ?? (icon ? <Icon name={icon} size={16} className="tw:shrink-0" /> : null)}
      <span className="tw:min-w-0 tw:flex-1 tw:py-1 tw:break-words">{label}</span>
      {count !== undefined ? (
        <span className="tw:shrink-0 tw:text-caption tw:text-muted-foreground tw:tabular-nums">{count.toLocaleString()}</span>
      ) : null}
    </ItemTrigger>
  );
}

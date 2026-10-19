import { useMemo, useState, type ReactNode } from "react";

import type { TagSummary } from "../../../ipc/cards";
import type { DeckSummary } from "../../../ipc/decks";
import type { DocumentSummary } from "../../../ipc/documents";
import { Icon, type IconName } from "../../../ui/icons/Icon";
import { IconButton } from "../../../ui/icons/IconButton";
import { cn } from "../../../ui/internal/cn";
import { OverflowMenu, type OverflowMenuItem } from "../../../ui/overlays/OverflowMenu";
import { Button } from "../../../ui/primitives/Button";
import { ItemTrigger } from "../../../ui/primitives/Item";
import { subjectColorIndexes, subjectInitials } from "../../home/homeModel";
import { SubjectBadge } from "../../home/SubjectBadge";
import {
  archivedDecks,
  sameScope,
  subjectNodes,
  type SubjectNode,
  type WorkbenchLibrary,
  type WorkbenchScope,
} from "./workbenchModel";

/** Tags shown before "Show all tags". */
const TAG_PREVIEW = 8;

export interface WorkbenchTreeActions {
  onSelectScope: (scope: WorkbenchScope) => void;
  onNewSubject: () => void;
  onRenameSubject: (subjectId: string) => void;
  onMoveSubject: (subjectId: string, offset: -1 | 1) => void;
  onDeleteSubject: (subjectId: string) => void;
  onNewDeck: (subjectId: string | null) => void;
  deckMenu: (deck: DeckSummary) => OverflowMenuItem[];
  documentMenu: (document: DocumentSummary) => OverflowMenuItem[];
  onOpenDocument: (document: DocumentSummary) => void;
  onCheckThese: () => void;
}

interface WorkbenchTreeProps extends WorkbenchTreeActions {
  library: WorkbenchLibrary;
  tags: TagSummary[];
  scope: WorkbenchScope;
  checkTheseCount: number;
  /** Phones show the tree as its own page with larger rows. */
  compact: boolean;
}

/**
 * The Workbench's left side: All cards, subjects with their decks and
 * documents, and tags. Choosing an entry scopes the card list; each subject,
 * deck and document carries its own actions menu.
 */
export function WorkbenchTree({ library, tags, scope, checkTheseCount, compact, ...actions }: WorkbenchTreeProps) {
  const nodes = useMemo(() => subjectNodes(library), [library]);
  const archived = useMemo(() => archivedDecks(library), [library]);
  const colors = useMemo(() => subjectColorIndexes(library.subjects), [library.subjects]);
  const totalCards = useMemo(
    () => library.decks.filter((deck) => deck.archivedAt === null).reduce((sum, deck) => sum + deck.generatedCardCount, 0),
    [library.decks],
  );
  const activeSubjectId = scope.kind === "subject"
    ? scope.subjectId
    : scope.kind === "deck"
      ? (library.organization?.decks.find((placement) => placement.deckId === scope.deckId)?.subjectId ?? null)
      : undefined;
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  // Few subjects open expanded; otherwise the one holding the current scope does.
  const [initialized, setInitialized] = useState(false);
  if (!initialized && library.organization !== null && nodes.length > 0) {
    setInitialized(true);
    setExpanded(new Set(nodes.length <= 3 ? nodes.map(nodeKey) : activeSubjectId !== undefined ? [String(activeSubjectId)] : []));
  }
  const [revealedSubject, setRevealedSubject] = useState(activeSubjectId);
  if (activeSubjectId !== revealedSubject) {
    setRevealedSubject(activeSubjectId);
    if (activeSubjectId !== undefined && !expanded.has(String(activeSubjectId))) {
      setExpanded(new Set([...expanded, String(activeSubjectId)]));
    }
  }
  const [showArchived, setShowArchived] = useState(false);
  const [showAllTags, setShowAllTags] = useState(false);
  const sortedTags = useMemo(
    () => [...tags].sort((left, right) => right.sourceNoteCount - left.sourceNoteCount || left.name.localeCompare(right.name)),
    [tags],
  );
  const visibleTags = showAllTags ? sortedTags : sortedTags.slice(0, TAG_PREVIEW);

  function toggle(node: SubjectNode): void {
    setExpanded((current) => {
      const next = new Set(current);
      const key = nodeKey(node);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  return (
    <nav aria-label="Library" data-tour-anchor="library.workbench" className="tw:grid tw:min-w-0 tw:content-start tw:gap-5 tw:pb-4">
      <div className="tw:grid tw:min-w-0">
        <TreeEntry
          icon="cards"
          label="All cards"
          count={totalCards}
          current={sameScope(scope, { kind: "view", view: "all" })}
          compact={compact}
          onSelect={() => actions.onSelectScope({ kind: "view", view: "all" })}
        />
      </div>

      <TreeSection
        title="Subjects"
        action={<IconButton icon="plus" label="New subject" onClick={actions.onNewSubject} />}
      >
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
        {nodes.length === 0 ? (
          <div className="tw:grid tw:justify-items-start tw:gap-2 tw:px-2">
            <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
              Subjects hold a course's decks and documents.
            </p>
            {/* Phones open on this page, so an empty library starts its first deck here. */}
            {compact ? <Button variant="primary" onClick={() => actions.onNewDeck(null)}>New deck</Button> : null}
          </div>
        ) : null}
        {nodes.map((node, index) => {
          const key = nodeKey(node);
          const open = expanded.has(key);
          const subjectId = node.subjectId;
          const badge = {
            subjectId,
            initials: subjectInitials(node.name),
            colorIndex: subjectId === null ? null : (colors.get(subjectId) ?? null),
          };
          const menu: OverflowMenuItem[] = [
            { id: "new-deck", label: "New deck here", onSelect: () => actions.onNewDeck(subjectId) },
          ];
          if (subjectId !== null) {
            menu.push(
              { id: "rename", label: "Rename", onSelect: () => actions.onRenameSubject(subjectId) },
              { id: "up", label: "Move up", disabled: index === 0, onSelect: () => actions.onMoveSubject(subjectId, -1) },
              {
                id: "down",
                label: "Move down",
                disabled: index >= library.subjects.length - 1,
                onSelect: () => actions.onMoveSubject(subjectId, 1),
              },
              { id: "delete", label: "Delete subject", destructive: true, onSelect: () => actions.onDeleteSubject(subjectId) },
            );
          }
          return (
            <div key={key} className="tw:grid tw:min-w-0" data-subject-id={subjectId ?? "none"}>
              <div className="tw:flex tw:min-w-0 tw:items-center tw:gap-0.5">
                <IconButton
                  icon={open ? "chevron-down" : "chevron-right"}
                  label={open ? `Collapse ${node.name}` : `Expand ${node.name}`}
                  aria-expanded={open}
                  iconSize={16}
                  tooltip={false}
                  onClick={() => toggle(node)}
                  className="tw:shrink-0"
                />
                <TreeEntry
                  leading={<SubjectBadge subject={badge} size="small" className="tw:size-6 tw:text-caption" />}
                  label={node.name}
                  count={node.cardCount}
                  current={sameScope(scope, { kind: "subject", subjectId })}
                  compact={compact}
                  onSelect={() => actions.onSelectScope({ kind: "subject", subjectId })}
                />
                <OverflowMenu label={`${node.name} actions`} items={menu} className="tw:shrink-0" />
              </div>
              {open ? (
                <div role="group" aria-label={node.name} className="tw:ml-8 tw:grid tw:min-w-0 tw:gap-px tw:border-l tw:border-solid tw:border-border tw:pl-1.5">
                  {node.decks.map((deck) => (
                    <div key={deck.id} className="tw:flex tw:min-w-0 tw:items-center">
                      <TreeEntry
                        icon="cards"
                        label={deck.name}
                        count={deck.generatedCardCount}
                        current={sameScope(scope, { kind: "deck", deckId: deck.id })}
                        compact={compact}
                        onSelect={() => actions.onSelectScope({ kind: "deck", deckId: deck.id })}
                      />
                      <OverflowMenu label={`${deck.name} actions`} items={actions.deckMenu(deck)} className="tw:shrink-0" />
                    </div>
                  ))}
                  {node.documents.map((document) => (
                    <div key={document.id} className="tw:flex tw:min-w-0 tw:items-center">
                      <TreeEntry
                        icon="document"
                        label={document.title}
                        muted
                        compact={compact}
                        onSelect={() => actions.onOpenDocument(document)}
                      />
                      <OverflowMenu label={`${document.title} actions`} items={actions.documentMenu(document)} className="tw:shrink-0" />
                    </div>
                  ))}
                  {node.decks.length === 0 && node.documents.length === 0 ? (
                    <p className="tw:m-0 tw:px-2 tw:py-1 tw:text-small tw:text-muted-foreground">Nothing here yet.</p>
                  ) : null}
                </div>
              ) : null}
            </div>
          );
        })}
        {archived.length > 0 ? (
          <div className="tw:grid tw:min-w-0">
            <Button variant="ghost" size="small" aria-expanded={showArchived} className="tw:justify-start tw:text-muted-foreground" onClick={() => setShowArchived((value) => !value)}>
              <Icon name="archive" size={16} />
              {showArchived ? "Hide archived decks" : `Archived decks (${archived.length.toLocaleString()})`}
            </Button>
            {showArchived ? archived.map((deck) => (
              <div key={deck.id} className="tw:ml-8 tw:flex tw:min-w-0 tw:items-center">
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
              current={sameScope(scope, { kind: "tag", tagId: tag.id })}
              compact={compact}
              onSelect={() => actions.onSelectScope({ kind: "tag", tagId: tag.id })}
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

function nodeKey(node: Pick<SubjectNode, "subjectId">): string {
  return String(node.subjectId);
}

function TreeSection({ title, action, children }: { title: string; action?: ReactNode; children: ReactNode }) {
  return (
    <section aria-label={title} className="tw:grid tw:min-w-0 tw:gap-0.5">
      <div className="tw:flex tw:min-h-8 tw:items-center tw:justify-between tw:gap-2 tw:px-2">
        <h2 className="tw:m-0 tw:text-caption tw:font-semibold tw:tracking-wide tw:text-muted-foreground tw:uppercase">{title}</h2>
        {action}
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
}

function TreeEntry({ label, icon, leading, count, current = false, muted = false, compact, onSelect }: TreeEntryProps) {
  return (
    <ItemTrigger
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

import type { MouseEvent, ReactNode } from "react";
import { useId } from "react";

import { StudyMarkdown } from "../../components/StudyMarkdown";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import type { GuideBlock, GuideReadingOrder, GuideSectionEntry } from "./guideModel";

/** Where a learner can go in the reader: a page, or a block on the current section page. */
export type GuideDestination =
  | { kind: "page"; topicId: string | null }
  | { kind: "block"; id: string };

/** The id of a page's article, which its outline link points at. */
export function guidePageId(topicId: string | null): string {
  return topicId === null ? "guide-page-overview" : `guide-page-${topicId}`;
}

/** The id prefix for the headed blocks of a section page. */
export function guideBlockPrefix(topicId: string): string {
  return `guide-${topicId}`;
}

const linkBase = "tw:flex tw:min-h-10 tw:min-w-0 tw:items-center tw:gap-2.5 tw:rounded-md tw:px-2 tw:py-1.5 tw:no-underline tw:outline-none tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:reduce-motion:transition-none tw:coarse:min-h-touch";
const otherLink = "tw:text-(--ostudy-color-text-secondary) tw:hover:bg-muted tw:hover:text-foreground";
const currentLink = "tw:bg-accent tw:font-semibold tw:text-foreground";
const marker = "tw:grid tw:size-6 tw:shrink-0 tw:place-items-center tw:rounded-full tw:border tw:border-solid tw:text-caption tw:tabular-nums";

/**
 * The guide's contents: the overview and every section, grouped under editorial chapters when the
 * guide has them. The current section opens to its headed blocks, which jump within the page.
 */
export function GuideOutline({ order, currentTopicId, readTopicIds, blocks, onNavigate, showLabel = true }: {
  order: GuideReadingOrder;
  /** The section being read, or null on the overview. */
  currentTopicId: string | null;
  /** Sections the learner has read, which show a check instead of their number. */
  readTopicIds: ReadonlySet<string>;
  /** The headed blocks of the current section, in page order. */
  blocks: GuideBlock[];
  onNavigate: (destination: GuideDestination) => void;
  /** The wide layout names the list; a sheet titled Contents does not need to. */
  showLabel?: boolean;
}) {
  const labelPrefix = `guide-outline-${useId().replaceAll(":", "")}`;
  function follow(event: MouseEvent<HTMLAnchorElement>, destination: GuideDestination): void {
    event.preventDefault();
    onNavigate(destination);
  }
  function sectionItem(entry: GuideSectionEntry): ReactNode {
    const topicId = entry.section.topicId;
    const current = topicId === currentTopicId;
    const read = readTopicIds.has(topicId);
    const prefix = guideBlockPrefix(topicId);
    return (
      <li key={topicId} className="tw:min-w-0">
        <a href={`#${guidePageId(topicId)}`} aria-current={current ? "page" : undefined}
          className={cn(linkBase, current ? currentLink : otherLink)}
          onClick={(event) => follow(event, { kind: "page", topicId })}>
          <span aria-hidden="true" className={cn(marker, current
            ? "tw:border-transparent tw:bg-primary tw:font-semibold tw:text-primary-foreground"
            : read ? "tw:border-success/30 tw:bg-success-soft tw:text-success"
            : "tw:border-border-strong tw:text-muted-foreground")}>
            {read && !current ? <Icon name="check" size={14} /> : entry.number}
          </span>
          <span className="tw:min-w-0 tw:flex-1 tw:break-words"><StudyMarkdown text={entry.title} inline /></span>
          {read ? <span className="sr-only">, read</span> : null}
        </a>
        {current && blocks.length > 0 ? (
          <ol aria-label="In this section"
            className="tw:mt-1 tw:mb-2 tw:ml-5 tw:grid tw:list-none tw:border-0 tw:border-l-2 tw:border-solid tw:border-border tw:py-0 tw:pr-0 tw:pl-2.5">
            {blocks.map((block) => (
              <li key={block.key} className="tw:min-w-0">
                <a href={`#${prefix}-${block.key}`}
                  className={cn(linkBase, otherLink, "tw:min-h-9 tw:text-label")}
                  onClick={(event) => follow(event, { kind: "block", id: `${prefix}-${block.key}` })}>
                  <span className="tw:min-w-0 tw:flex-1 tw:break-words"><StudyMarkdown text={block.label} inline /></span>
                </a>
              </li>
            ))}
          </ol>
        ) : null}
      </li>
    );
  }
  return (
    <nav aria-label="Study guide contents" className="tw:grid tw:min-w-0 tw:gap-1 tw:font-sans tw:text-small">
      {showLabel ? (
        <p className="tw:m-0 tw:mb-1 tw:px-2 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground">Contents</p>
      ) : null}
      <ol className="tw:m-0 tw:grid tw:list-none tw:gap-0.5 tw:p-0">
        <li className="tw:min-w-0">
          <a href={`#${guidePageId(null)}`} aria-current={currentTopicId === null ? "page" : undefined}
            className={cn(linkBase, currentTopicId === null ? currentLink : otherLink)}
            onClick={(event) => follow(event, { kind: "page", topicId: null })}>
            <span aria-hidden="true" className={cn(marker, currentTopicId === null
              ? "tw:border-transparent tw:bg-primary tw:text-primary-foreground"
              : "tw:border-border-strong tw:text-muted-foreground")}>
              <Icon name="book" size={14} />
            </span>
            <span className="tw:min-w-0 tw:flex-1">Overview</span>
          </a>
        </li>
        {order.groups.map((group) => group.title === null ? group.entries.map(sectionItem) : (
          <li key={group.id} className="tw:mt-3 tw:min-w-0">
            <p id={`${labelPrefix}-${group.id}`}
              className="tw:m-0 tw:mb-1 tw:px-2 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground">
              <StudyMarkdown text={group.title} inline />
            </p>
            <ol aria-labelledby={`${labelPrefix}-${group.id}`} className="tw:m-0 tw:grid tw:list-none tw:gap-0.5 tw:p-0">
              {group.entries.map(sectionItem)}
            </ol>
          </li>
        ))}
      </ol>
    </nav>
  );
}

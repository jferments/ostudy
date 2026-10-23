import { type ReactNode, type RefObject, useEffect, useId, useRef, useState } from "react";

import { StudyMarkdown } from "../../components/StudyMarkdown";
import type { DocumentStudyGuide, StudyGuideOverlay, StudyGuidePdfTheme, TopicCardProjection } from "../../ipc/studyGuides";
import type { TutorTarget } from "../../ipc/tutor";
import { Icon } from "../../ui/icons/Icon";
import { IconButton } from "../../ui/icons/IconButton";
import { cn } from "../../ui/internal/cn";
import type { LayoutMode } from "../../ui/layout/LayoutModeProvider";
import { OverflowMenu, type OverflowMenuItem } from "../../ui/overlays/OverflowMenu";
import { Badge } from "../../ui/primitives/Badge";
import { Button } from "../../ui/primitives/Button";
import { Card, CardContent, CardHeader, CardTitle } from "../../ui/primitives/Card";
import { Switch } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { ListRowMedia } from "../../ui/primitives/ListRow";
import { useOptionalTutor } from "../tutor/TutorContext";
import { type GuideDestination, guideBlockPrefix, guidePageId } from "./GuideOutline";
import {
  fileKindLabel,
  findConflictedOverride,
  findEditingOverlay,
  formatLocatorRange,
  formatMinutes,
  formatSourceLength,
  type GuideReadingOrder,
  type GuideSectionEntry,
  type GuideSourceSummary,
  plural,
  readingMinutes,
  sectionReadingText,
  splitOverviewConcept,
} from "./guideModel";
import { GuidePaper } from "./GuidePaper";
import { type CheckProgressMap, GuideMinuteBox, GuideTermList, StudyGuideStructuredContent } from "./StudyGuideContent";
import type { GuidePageStyle } from "./useDesignedSectionPage";
import type { InkMark } from "../../ipc/materialInk";

/** A neighbor page for the previous and next links. */
export interface GuidePageLink {
  topicId: string | null;
  number: number | null;
  title: string;
}

interface GuideSourceProps {
  documentId: string;
  documentTitle: string;
  source: GuideSourceSummary | null;
  onOpenSource: () => void;
}

/**
 * The reading column: one measure of the card font, centered in the space it has. Its scroll margin
 * is deeper than the space above it, so scrolling to a page lands at the top of the scroll area.
 */
const columnClass = "tw:mx-auto tw:flex tw:w-full tw:max-w-[42rem] tw:min-w-0 tw:scroll-mt-24 tw:flex-col tw:gap-7 tw:compact:gap-6";
const railClass = "tw:flex tw:min-w-0 tw:flex-col tw:gap-7 tw:pt-1 tw:font-sans";
// On wide screens the margin is a shaded panel, apart from the reading column.
const wideRailClass = "tw:flex tw:min-w-0 tw:flex-col tw:gap-6 tw:rounded-xl tw:bg-foreground/4 tw:p-4 tw:font-sans";
const railHeading = "tw:m-0 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground";
const pageHeading = "tw:m-0 tw:min-w-0 tw:scroll-mt-24 tw:break-words tw:text-title tw:font-semibold tw:tracking-tighter tw:text-foreground tw:outline-none tw:wide:text-display";

export function GuideOverviewPage({
  guide, guideTitle, order, layoutMode, pageStyle, guideNote, actionsDisabled, onNavigate, onEditNote, onRemoveNote, ...sourceProps
}: GuideSourceProps & {
  guide: DocumentStudyGuide;
  guideTitle: string;
  order: GuideReadingOrder;
  layoutMode: LayoutMode;
  /** The overview reads on a page of guide paper in this theme, or as plain text. */
  pageStyle: GuidePageStyle;
  guideNote: StudyGuideOverlay | null;
  actionsDisabled: boolean;
  onNavigate: (destination: GuideDestination) => void;
  onEditNote: () => void;
  onRemoveNote: (note: StudyGuideOverlay) => void;
}) {
  const ids = `guide-overview-${useId().replaceAll(":", "")}`;
  const headingId = `${guidePageId(null)}-title`;
  const chapters = order.groups.filter((group) => group.title !== null);
  const minutes = order.entries.reduce((total, entry) => total + readingMinutes(sectionReadingText(entry.section,
    findEditingOverlay(guide.overlays, entry.section.topicId, "override", guide.id))), 0);
  const counts = [chapters.length > 0 ? plural(chapters.length, "chapter") : null, plural(order.entries.length, "section"), formatMinutes(minutes)]
    .filter(Boolean).join(" · ");
  const first = order.entries[0];
  const concepts = guide.overview.keyConcepts.slice(0, 6);
  const wide = layoutMode === "wide";
  const paper = pageStyle === "plain" ? null : pageStyle;
  const aboutBlocks = <>
    <GuideNoteBlock label="Your note" note={guideNote} disabled={actionsDisabled}
      onEdit={onEditNote} onRemove={onRemoveNote} />
    <GuideSourceBlock {...sourceProps} detail={formatSourceLength(sourceProps.source)} />
  </>;
  return <>
    <article id={guidePageId(null)} aria-labelledby={headingId} className={columnClass}>
      <header className="tw:grid tw:gap-3 tw:font-sans">
        <p className="tw:m-0 tw:text-small tw:font-semibold tw:text-accent-foreground">{counts}</p>
        <h1 id={headingId} tabIndex={-1} className={pageHeading}><StudyMarkdown text={guideTitle} inline /></h1>
        <SourceChip {...sourceProps} detail={formatSourceLength(sourceProps.source)} />
      </header>
      <OnPaper paper={paper}>
      {guide.overview.summary.trim() ? (
        <GuideMinuteBox id={`${ids}-minute`} page paper={paper !== null}><StudyMarkdown text={guide.overview.summary} /></GuideMinuteBox>
      ) : null}
      {guide.overview.learningObjectives.length > 0 ? (
        <section aria-labelledby={`${ids}-objectives`} className="tw:grid tw:gap-3">
          <h2 id={`${ids}-objectives`} className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">Learning objectives</h2>
          <ul className="tw:m-0 tw:grid tw:list-disc tw:gap-2 tw:pl-6 tw:font-(family-name:--card-font-family) tw:text-body-large tw:leading-(--ostudy-font-line-height-reading) tw:marker:text-primary tw:compact:text-body">
            {guide.overview.learningObjectives.map((objective, index) => <li key={`${index}-${objective}`} className="tw:pl-1"><StudyMarkdown text={objective} /></li>)}
          </ul>
        </section>
      ) : null}
      {concepts.length > 0 ? (
        <section aria-labelledby={`${ids}-concepts`} className="tw:grid tw:gap-3">
          <h2 id={`${ids}-concepts`} className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">Key concepts</h2>
          <dl className="tw:m-0 tw:grid tw:font-sans tw:text-body">
            {concepts.map((concept, index) => {
              const [term, caption] = splitOverviewConcept(concept);
              // A term without a caption keeps the whole line instead of an empty second column.
              return <div key={`${index}-${concept}`}
                className={cn("tw:grid tw:min-w-0 tw:gap-0.5 tw:border-0 tw:border-t tw:border-solid tw:py-3 tw:first:border-t-0 tw:first:pt-0",
                  paper ? "tw:border-(--guide-rule)" : "tw:border-border", caption && "tw:wide:grid-cols-[minmax(0,13rem)_minmax(0,1fr)] tw:wide:gap-x-6")}>
                <dt className={cn("tw:font-semibold", paper ? "tw:text-(--guide-primary)" : "tw:text-foreground")}><StudyMarkdown text={term} inline /></dt>
                {caption ? <dd className={cn("tw:m-0", paper ? "tw:text-(--guide-ink)" : "tw:text-(--ostudy-color-text-secondary)")}><StudyMarkdown text={caption} /></dd> : null}
              </div>;
            })}
          </dl>
        </section>
      ) : null}
      </OnPaper>
      {chapters.length > 0 ? (
        <section aria-labelledby={`${ids}-chapters`} className="tw:grid tw:gap-3">
          <h2 id={`${ids}-chapters`} className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">Chapters</h2>
          <ol className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0">
            {chapters.map((chapter) => (
              <li key={chapter.id}>
                <Card as="section" aria-labelledby={`${ids}-chapter-${chapter.id}`}>
                  <CardHeader>
                    <CardTitle id={`${ids}-chapter-${chapter.id}`}><StudyMarkdown text={chapter.title ?? ""} inline /></CardTitle>
                    {chapter.introduction.trim() ? (
                      <div className="tw:col-start-1 tw:text-small tw:text-(--ostudy-color-text-secondary)"><StudyMarkdown text={chapter.introduction} /></div>
                    ) : null}
                  </CardHeader>
                  <CardContent>
                    <ol className="tw:m-0 tw:grid tw:list-none tw:gap-0.5 tw:p-0">
                      {chapter.entries.map((entry) => (
                        <li key={entry.section.topicId}>
                          <PageLink link={{ topicId: entry.section.topicId, number: entry.number, title: entry.title }} onNavigate={onNavigate} />
                        </li>
                      ))}
                    </ol>
                  </CardContent>
                </Card>
              </li>
            ))}
          </ol>
        </section>
      ) : null}
      {!wide ? <aside aria-label="About this guide" className={cn(railClass, "tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-6")}>{aboutBlocks}</aside> : null}
      <GuidePageNav onNavigate={onNavigate} previous={null} nextLabel="Start reading"
        next={first ? { topicId: first.section.topicId, number: first.number, title: first.title } : null} />
    </article>
    {wide ? <aside aria-label="About this guide" className={wideRailClass}>{aboutBlocks}</aside> : null}
  </>;
}

/** Reading text on a page of guide paper, or as it is when the guide reads plain. */
function OnPaper({ paper, children }: { paper: StudyGuidePdfTheme | null; children: ReactNode }) {
  if (paper === null) return <>{children}</>;
  return <GuidePaper theme={paper}><div className="tw:flex tw:min-w-0 tw:flex-col tw:gap-7 tw:compact:gap-6">{children}</div></GuidePaper>;
}

export function GuideSectionPage({
  guide, entry, sectionCount, layoutMode, pageStyle, marks, onWriteOnPage, projection, previous, next, checkProgress, onCheckProgressChange,
  actionsDisabled, onNavigate, read, onSetRead, onPractice, onEditNote, onRemoveNote, onEditSection, onResetSection, onDiscardConflict,
  ...sourceProps
}: GuideSourceProps & {
  guide: DocumentStudyGuide;
  entry: GuideSectionEntry;
  sectionCount: number;
  layoutMode: LayoutMode;
  /** Sections read on a page of guide paper in this theme, or as plain text. */
  pageStyle: GuidePageStyle;
  /** The learner's marks on this section's designed page, which opens to write on. */
  marks: readonly InkMark[];
  /** Opens the designed page to write on. */
  onWriteOnPage: () => void;
  projection: TopicCardProjection | undefined;
  previous: GuidePageLink | null;
  next: GuidePageLink | null;
  checkProgress: CheckProgressMap;
  onCheckProgressChange: (next: CheckProgressMap) => void;
  actionsDisabled: boolean;
  onNavigate: (destination: GuideDestination) => void;
  /** Whether the learner has read this section's current text. */
  read: boolean;
  onSetRead: (read: boolean) => void;
  onPractice: () => void;
  onEditNote: () => void;
  onRemoveNote: (note: StudyGuideOverlay) => void;
  onEditSection: () => void;
  onResetSection: (override: StudyGuideOverlay) => void;
  onDiscardConflict: (conflict: StudyGuideOverlay) => void;
}) {
  const tutor = useOptionalTutor();
  const { section, number, title } = entry;
  const topicId = section.topicId;
  const note = findEditingOverlay(guide.overlays, topicId, "note", guide.id);
  const override = findEditingOverlay(guide.overlays, topicId, "override", guide.id);
  const conflict = findConflictedOverride(guide.overlays, topicId);
  // A section the learner rewrote reads as their text, never as a page of the generated one.
  const paper = pageStyle === "plain" ? null : pageStyle;
  const minutes = readingMinutes(sectionReadingText(section, override));
  const range = formatLocatorRange(section.sourceLocator);
  const headingId = `${guidePageId(topicId)}-title`;
  const wide = layoutMode === "wide";
  const tutorTarget: TutorTarget = { title, scope: { kind: "guide", id: sourceProps.documentId },
    focus: { kind: "guide", documentId: sourceProps.documentId, guideId: guide.id, topicId, contentHash: section.contentHash }, mode: "explain" };
  const menuItems: OverflowMenuItem[] = [
    ...(tutor ? [{ id: "tutor", label: "Ask Tutor about this section", disabled: actionsDisabled || tutor.busy || tutor.pending !== null,
      onSelect: () => tutor.openContext(tutorTarget) }] : []),
    { id: "read", label: read ? "Mark as unread" : "Mark as read", onSelect: () => onSetRead(!read) },
    ...(paper !== null && !override
      ? [{ id: "write", label: marks.length > 0 ? "Open your writing on this page" : "Write on this page", disabled: actionsDisabled, onSelect: onWriteOnPage }] : []),
    { id: "note", label: note ? "Edit note" : "Add note", disabled: actionsDisabled, onSelect: onEditNote },
    { id: "edit", label: "Edit or rewrite this section", disabled: actionsDisabled, onSelect: onEditSection },
    ...(note ? [{ id: "remove-note", label: "Remove note", destructive: true, disabled: actionsDisabled, onSelect: () => onRemoveNote(note) }] : []),
    ...(override ? [{ id: "reset-section", label: "Reset section edit", destructive: true, disabled: actionsDisabled,
      onSelect: () => onResetSection(override) }] : []),
  ];
  const cardCount = projection?.cardCount ?? null;
  // On phones the bottom bar gives way once the section's end, with its own practice and next
  // links, comes on screen.
  const endRef = useRef<HTMLDivElement>(null);
  const endOnScreen = useOnScreen(endRef, layoutMode === "compact");
  const [hideMeanings, setHideMeanings] = useState(readHideTermMeanings);
  const chooseHideMeanings = (hide: boolean) => {
    setHideMeanings(hide);
    saveHideTermMeanings(hide);
  };
  const aboutBlocks = <>
    <GuideNoteBlock label="Your note" note={note} disabled={actionsDisabled}
      onEdit={onEditNote} onRemove={onRemoveNote} />
    {section.content.importantTerms.length > 0 && !override ? (
      <RailSection title="Important terms" action={
        <label className="tw:flex tw:items-center tw:gap-2 tw:text-small tw:text-muted-foreground">
          <Switch checked={hideMeanings} onCheckedChange={chooseHideMeanings} />
          Hide meanings
        </label>
      }>
        <GuideTermList terms={section.content.importantTerms} className="tw:text-small" hideMeanings={hideMeanings} />
      </RailSection>
    ) : null}
    <GuideSourceBlock {...sourceProps} detail={range} />
  </>;
  return <>
    <article id={guidePageId(topicId)} aria-labelledby={headingId} className={columnClass}>
      <header className="tw:grid tw:gap-3 tw:font-sans">
        <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-x-2 tw:gap-y-1">
          <p className="tw:m-0 tw:text-small tw:font-semibold tw:text-accent-foreground">
            Section {number} of {sectionCount} · {formatMinutes(minutes)}
          </p>
          {override ? <Badge tone="neutral">Edited by you</Badge> : null}
        </div>
        <div className="tw:flex tw:min-w-0 tw:items-start tw:gap-2">
          <h1 id={headingId} tabIndex={-1} className={cn(pageHeading, "tw:flex-1")}><StudyMarkdown text={title} inline /></h1>
          <OverflowMenu label={`Section actions for ${title}`} items={menuItems} className="tw:mt-1 tw:shrink-0" />
        </div>
        <SourceChip {...sourceProps} detail={range} />
      </header>
      {conflict ? (
        <InlineNotice kind="warning" title="Older section edit needs review.">
          <p className="tw:mt-0">The generated base changed, so ostudy did not silently apply the old override to new source-grounded content. The prior edit remains preserved below as a conflict.</p>
          <div className="tw:my-3 tw:[&_p]:whitespace-pre-wrap"><StudyMarkdown text={conflict.plainText} /></div>
          <Button variant="secondary" disabled={actionsDisabled} onClick={() => onDiscardConflict(conflict)}>Discard older edit</Button>
        </InlineNotice>
      ) : null}
      {override ? (
        <OnPaper paper={paper}>
          <div className="tw:min-w-0 tw:break-words tw:font-(family-name:--card-font-family) tw:text-body-large tw:leading-(--ostudy-font-line-height-reading) tw:compact:text-body tw:[&_p]:my-4 tw:[&_p]:whitespace-pre-wrap">
            <StudyMarkdown text={override.plainText} />
          </div>
        </OnPaper>
      ) : (
        <StudyGuideStructuredContent content={section.content} headingLevel={2} idPrefix={guideBlockPrefix(topicId)}
          sectionNumber={number} showTerms={false} figureSource={{ documentId: sourceProps.documentId, locator: section.sourceLocator }}
          checkProgress={checkProgress} onCheckProgressChange={onCheckProgressChange} paper={paper} />
      )}
      <div ref={endRef} className="tw:flex tw:min-w-0 tw:flex-col tw:gap-7 tw:compact:gap-6">
        {cardCount === null ? null : cardCount > 0 ? (
          <Card as="section" aria-labelledby={`${headingId}-cards`} className="tw:font-sans">
            <CardContent className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-x-5 tw:gap-y-3">
              <div className="tw:min-w-0 tw:flex-[1_1_16rem]">
                <h2 id={`${headingId}-cards`} className="tw:m-0 tw:text-body-large tw:font-semibold tw:tracking-tight">
                  {plural(cardCount, "card")} {cardCount === 1 ? "comes" : "come"} from this section
                </h2>
                <p className="tw:mt-0.5 tw:mb-0 tw:text-small tw:text-muted-foreground">They are the same cards as in your decks, not copies.</p>
              </div>
              <Button variant="primary" size="large" disabled={actionsDisabled} onClick={onPractice}>
                <Icon name="practice" size={16} />Practice {plural(cardCount, "card")}
              </Button>
            </CardContent>
          </Card>
        ) : (
          <p className="tw:m-0 tw:font-sans tw:text-small tw:text-muted-foreground">No flashcards come from this section yet.</p>
        )}
        {/* Narrower screens have no margin, so its blocks follow the section's end. */}
        {!wide ? <aside aria-label="About this section" className={cn(railClass, "tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-6")}>{aboutBlocks}</aside> : null}
        <GuidePageNav previous={previous} next={next} onNavigate={onNavigate} />
        {/* The last section has no Next to count it read, so it says so itself. */}
        {next === null ? (
          read ? (
            <p className="tw:m-0 tw:flex tw:items-center tw:gap-2 tw:font-sans tw:text-small tw:font-semibold tw:text-success">
              <Icon name="check" size={16} />Read
            </p>
          ) : (
            <Button variant="primary" className="tw:self-start" onClick={() => onSetRead(true)}>
              <Icon name="check" size={16} />Mark as read
            </Button>
          )
        ) : null}
      </div>
      {layoutMode === "compact" ? (
        <GuideBottomBar previous={previous} next={next} cardCount={cardCount} position={`Section ${number} of ${sectionCount}`}
          hidden={endOnScreen} disabled={actionsDisabled} onNavigate={onNavigate} onPractice={onPractice} />
      ) : null}
    </article>
    {wide ? (
      <aside aria-label="About this section" className={wideRailClass}>
        {aboutBlocks}
        {cardCount !== null && cardCount > 0 ? (
          <Card as="section" size="sm" aria-labelledby={`${headingId}-rail-cards`}>
            <CardHeader>
              <CardTitle as="h2" id={`${headingId}-rail-cards`}>{plural(cardCount, "card")} from this section</CardTitle>
            </CardHeader>
            <CardContent>
              <Button variant="secondary" className="tw:w-full" disabled={actionsDisabled} onClick={onPractice}>
                <Icon name="practice" size={16} />Practice {plural(cardCount, "card")}
              </Button>
            </CardContent>
          </Card>
        ) : null}
      </aside>
    ) : null}
  </>;
}

const HIDE_MEANINGS_KEY = "ostudy.guideHideTermMeanings";

/** Whether term meanings were last hidden on this device, so the learner can test themselves. */
function readHideTermMeanings(): boolean {
  try {
    return window.localStorage.getItem(HIDE_MEANINGS_KEY) === "true";
  } catch {
    return false;
  }
}

function saveHideTermMeanings(hide: boolean): void {
  try {
    window.localStorage.setItem(HIDE_MEANINGS_KEY, String(hide));
  } catch {
    // A convenience only; the switch still works for this page.
  }
}

function RailSection({ title, action = null, children }: { title: string; action?: ReactNode; children: ReactNode }) {
  const id = `guide-rail-${useId().replaceAll(":", "")}`;
  return (
    <section aria-labelledby={id} className="tw:grid tw:min-w-0 tw:gap-2.5">
      {action ? (
        <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-2">
          <h2 id={id} className={railHeading}>{title}</h2>
          {action}
        </div>
      ) : <h2 id={id} className={railHeading}>{title}</h2>}
      {children}
    </section>
  );
}

function GuideNoteBlock({ label, note, disabled, onEdit, onRemove }: {
  label: string;
  note: StudyGuideOverlay | null;
  disabled: boolean;
  onEdit: () => void;
  onRemove: (note: StudyGuideOverlay) => void;
}) {
  return (
    <RailSection title={label}>
      {note ? <>
        <div className="tw:min-w-0 tw:break-words tw:rounded-md tw:bg-highlight tw:px-4 tw:py-3 tw:text-small tw:text-foreground tw:[&_p]:whitespace-pre-wrap">
          <StudyMarkdown text={note.plainText} />
        </div>
        <div className="tw:flex tw:flex-wrap tw:gap-1">
          <Button variant="ghost" size="small" disabled={disabled} onClick={onEdit}><Icon name="edit" size={16} />Edit note</Button>
          <Button variant="dangerGhost" size="small" disabled={disabled} onClick={() => onRemove(note)}>Remove note</Button>
        </div>
      </> : <>
        <Button variant="secondary" size="small" className="tw:justify-self-start" disabled={disabled} onClick={onEdit}>
          <Icon name="edit" size={16} />Add note
        </Button>
      </>}
    </RailSection>
  );
}

function GuideSourceBlock({ documentTitle, source, detail, onOpenSource }: GuideSourceProps & { detail: string | null }) {
  const kind = fileKindLabel(source);
  return (
    <RailSection title="Source">
      <div className="tw:flex tw:min-w-0 tw:items-center tw:gap-3">
        {kind ? <ListRowMedia label={kind} /> : <ListRowMedia icon="document" />}
        <div className="tw:grid tw:min-w-0 tw:gap-0.5">
          <p className="tw:m-0 tw:break-words tw:text-small tw:font-semibold tw:text-foreground">{documentTitle}</p>
          {detail ? <p className="tw:m-0 tw:text-label tw:text-muted-foreground">{detail}</p> : null}
        </div>
      </div>
      <Button variant="secondary" size="small" className="tw:justify-self-start" onClick={onOpenSource}>Open source</Button>
    </RailSection>
  );
}

/** "From [PDF] Lecture 4 · pp. 6–8": names the source and opens it. */
function SourceChip({ documentTitle, source, detail, onOpenSource }: GuideSourceProps & { detail: string | null }) {
  const kind = fileKindLabel(source);
  const text = detail ? `${documentTitle} · ${detail}` : documentTitle;
  return (
    <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2">
      <span className="tw:text-small tw:text-muted-foreground">From</span>
      <Button variant="secondary" className="tw:max-w-full tw:min-w-0 tw:pl-1.5 tw:font-medium" aria-label={`Open source: ${text}`} onClick={onOpenSource}>
        {kind ? <Badge tone="info" aria-hidden="true">{kind}</Badge> : <Icon name="document" size={16} />}
        <span className="tw:min-w-0 tw:truncate">{text}</span>
      </Button>
    </div>
  );
}

function PageLink({ link, label, align = "start", primary = false, onNavigate }: {
  link: GuidePageLink;
  label?: string;
  align?: "start" | "end";
  /** The way on through the guide is the page's main action. */
  primary?: boolean;
  onNavigate: (destination: GuideDestination) => void;
}) {
  return (
    <a href={`#${guidePageId(link.topicId)}`}
      className={cn(
        "tw:grid tw:min-w-0 tw:max-w-full tw:gap-0.5 tw:no-underline tw:outline-none tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:reduce-motion:transition-none",
        primary
          ? "tw:rounded-xl tw:bg-primary tw:px-4 tw:py-2.5 tw:text-primary-foreground tw:shadow-sm tw:hover:bg-primary/90"
          : "tw:rounded-md tw:px-2 tw:py-1.5 tw:hover:bg-muted",
        align === "end" ? cn("tw:ml-auto tw:justify-items-end tw:text-right", !primary && "tw:-mr-2") : "tw:-ml-2",
      )}
      onClick={(event) => { event.preventDefault(); onNavigate({ kind: "page", topicId: link.topicId }); }}>
      {/* The space keeps "Next" and the title apart in the link's name; the grid does not draw it. */}
      {label ? <><span className={cn("tw:text-label", primary ? "tw:text-primary-foreground/85" : "tw:text-muted-foreground")}>{label}</span>{" "}</> : null}
      <span className={cn("tw:break-words tw:font-semibold", primary ? "tw:text-primary-foreground" : "tw:text-foreground")}>
        {link.number === null ? null : <><span className="tw:tabular-nums">{link.number}</span>{" · "}</>}
        <StudyMarkdown text={link.title} inline />
      </span>
    </a>
  );
}

function GuidePageNav({ previous, next, nextLabel = "Next", onNavigate }: {
  previous: GuidePageLink | null;
  next: GuidePageLink | null;
  nextLabel?: string;
  onNavigate: (destination: GuideDestination) => void;
}) {
  if (!previous && !next) return null;
  return (
    <nav aria-label="Previous and next" className="tw:flex tw:flex-wrap tw:items-start tw:gap-3 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-5 tw:font-sans">
      {previous ? <PageLink link={previous} label="Previous" onNavigate={onNavigate} /> : null}
      {next ? <PageLink link={next} label={nextLabel} align="end" primary onNavigate={onNavigate} /> : null}
    </nav>
  );
}

/** Whether part of an element is on screen. Without an observer it reports false. */
function useOnScreen(ref: RefObject<Element | null>, enabled: boolean): boolean {
  const [onScreen, setOnScreen] = useState(false);
  useEffect(() => {
    const element = ref.current;
    if (!enabled || !element || typeof IntersectionObserver === "undefined") return undefined;
    const observer = new IntersectionObserver((entries) => {
      const latest = entries[entries.length - 1];
      if (latest) setOnScreen(latest.isIntersecting);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref, enabled]);
  return enabled && onScreen;
}

/** Phones keep the neighbors and the section's practice within thumb reach while reading. */
function GuideBottomBar({ previous, next, cardCount, position, hidden, disabled, onNavigate, onPractice }: {
  previous: GuidePageLink | null;
  next: GuidePageLink | null;
  cardCount: number | null;
  position: string;
  /** Hidden bars keep their place, so the page does not jump at its end. */
  hidden: boolean;
  disabled: boolean;
  onNavigate: (destination: GuideDestination) => void;
  onPractice: () => void;
}) {
  return (
    <nav aria-label="Section actions" aria-hidden={hidden || undefined} inert={hidden || undefined}
      className={cn("tw:sticky tw:bottom-3 tw:z-(--ostudy-layer-sticky) tw:flex tw:items-center tw:gap-1.5 tw:rounded-full tw:border tw:border-solid tw:border-border tw:bg-card tw:p-1.5 tw:font-sans tw:shadow-md tw:transition-[opacity,visibility] tw:duration-(--ostudy-motion-duration-fast) tw:reduce-motion:transition-none",
        hidden && "tw:invisible tw:opacity-0")}>
      {previous ? (
        <IconButton icon="chevron-left" tooltip={false} className="tw:size-12 tw:rounded-full"
          label={previous.number === null ? `Previous: ${previous.title}` : `Previous section: ${previous.title}`}
          onClick={() => onNavigate({ kind: "page", topicId: previous.topicId })} />
      ) : <span aria-hidden="true" className="tw:size-12 tw:shrink-0" />}
      <div className="tw:flex tw:min-w-0 tw:flex-1 tw:justify-center">
        {cardCount !== null && cardCount > 0 ? (
          <Button variant="primary" size="large" className="tw:h-12 tw:w-full" disabled={disabled} onClick={onPractice}>
            <Icon name="practice" size={16} />Practice {plural(cardCount, "card")}
          </Button>
        ) : <span className="tw:text-small tw:text-muted-foreground">{position}</span>}
      </div>
      {next ? (
        <IconButton icon="chevron-right" tooltip={false} className="tw:size-12 tw:rounded-full"
          label={`Next section: ${next.title}`}
          onClick={() => onNavigate({ kind: "page", topicId: next.topicId })} />
      ) : <span aria-hidden="true" className="tw:size-12 tw:shrink-0" />}
    </nav>
  );
}

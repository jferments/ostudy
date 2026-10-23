import { useEffect, useId, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { useOptionalHomeSummary } from "../../controllers/HomeSummaryProvider";
import { renderDocumentPageTile } from "../../ipc/documentAnnotations";
import type { SubjectCover } from "../../ipc/home";
import type { LibrarySubjectScope, SubjectSection } from "../../navigation/appDestinations";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { PageHeader, PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { Button } from "../../ui/primitives/Button";
import { ItemTrigger } from "../../ui/primitives/Item";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { cropTile } from "../documents/figures/cropTile";
import { buildSubjectTiles, countLabel, SUBJECT_COLORS, type SubjectTile } from "../home/homeModel";
import { NewSubjectSheet } from "../home/NewSubjectSheet";
import { MAX_SESSION_DECKS } from "../home/TodayStrip";
import { useDocumentPreparations } from "../home/useDocumentPreparations";
import { SubjectActions, useLibrarySubjects } from "./LibrarySubjects";

interface LibraryHomeProps {
  /** Opens one of a subject's pages; `null` is No subject. */
  onOpenSubject: (subjectId: string | null, page: SubjectSection) => void;
  onChooseSubject: (subject: LibrarySubjectScope) => void;
  /** Opens the Workbench on every card in the library. */
  onOpenAllCards: () => void;
  onStartReview: (deckIds: string[]) => void;
  showToast: ShowToast;
}

const ALL: LibrarySubjectScope = { kind: "all" };

/**
 * Library's own page: every subject as a tile with its figure, what it holds
 * and a button for each kind of material in it. A subject's pages open from
 * its tile; Library has no tabs of its own.
 */
export function LibraryHome({ onOpenSubject, onChooseSubject, onOpenAllCards, onStartReview, showToast }: LibraryHomeProps) {
  const home = useOptionalHomeSummary();
  const subjects = useLibrarySubjects(ALL);
  const preparations = useDocumentPreparations();
  const [creating, setCreating] = useState(false);
  const headingId = useId();

  if (!subjects.ready || !home?.summary || !home.library) {
    return (
      <PageScaffold data-testid="library-home">
        <LoadingState label="Opening Library…" />
      </PageScaffold>
    );
  }
  const { subjects: tiles, noSubject } = buildSubjectTiles(home.summary, home.library, preparations);
  const shown = noSubject ? [...tiles, noSubject] : tiles;
  const cards = shown.reduce((sum, tile) => sum + tile.summary.cardCount, 0);
  const due = shown.reduce((sum, tile) => sum + tile.dueCount, 0);
  const dueDeckIds = [...new Set(shown.flatMap((tile) => tile.summary.dueDeckIds))];

  return (
    <PageScaffold data-testid="library-home">
      <PageHeader
        title={<span id={headingId}>Library</span>}
        description={[countLabel(tiles.length, "subject"), countLabel(cards, "card")].join(" · ")}
        actions={
          <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2">
            {due > 0 ? (
              <Button
                data-testid="library-review"
                disabled={dueDeckIds.length > MAX_SESSION_DECKS}
                onClick={() => onStartReview(dueDeckIds)}
              >
                <Icon name="practice" size={15} /> Review {due.toLocaleString()} due
              </Button>
            ) : null}
            <Button data-testid="library-all-cards" onClick={onOpenAllCards}>
              <Icon name="cards" size={15} /> All cards
            </Button>
            <Button variant="primary" data-testid="library-new-subject" onClick={() => setCreating(true)}>
              <Icon name="plus" size={16} /> New subject
            </Button>
          </div>
        }
      />
      <ul
        aria-labelledby={headingId}
        data-testid="library-subject-tiles"
        className="tw:m-0 tw:grid tw:list-none tw:grid-cols-[repeat(auto-fill,minmax(18rem,1fr))] tw:gap-5 tw:p-0 tw:compact:grid-cols-1 tw:compact:gap-3"
      >
        {shown.map((tile) => (
          <li key={tile.key} className="tw:min-w-0">
            <LibrarySubjectTile
              tile={tile}
              onOpen={(page) => onOpenSubject(tile.subjectId, page)}
              actions={
                tile.subjectId !== null ? (
                  <SubjectActions
                    subjectId={tile.subjectId}
                    name={tile.name}
                    subjects={subjects}
                    onChooseSubject={onChooseSubject}
                    showToast={showToast}
                  />
                ) : null
              }
            />
          </li>
        ))}
        <li className="tw:min-w-0">
          <ItemTrigger
            data-testid="library-new-subject-tile"
            onClick={() => setCreating(true)}
            className="tw:flex tw:size-full tw:min-h-40 tw:cursor-pointer tw:flex-col tw:items-center tw:justify-center tw:gap-2 tw:rounded-lg tw:border-[1.5px] tw:border-dashed tw:border-border tw:bg-transparent tw:font-sans tw:text-body tw:font-semibold tw:text-primary tw:outline-none tw:transition-colors tw:hover:bg-foreground/4 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:compact:min-h-16 tw:compact:flex-row"
          >
            <Icon name="plus" size={22} /> New subject
          </ItemTrigger>
        </li>
      </ul>
      {creating ? (
        <NewSubjectSheet
          onClose={() => setCreating(false)}
          onCreated={(library, name) => {
            setCreating(false);
            home.acceptLibrary(library);
            home.refresh();
            const created = library.subjects.find((subject) => subject.name === name);
            if (created) onOpenSubject(created.id, "documents");
          }}
        />
      ) : null}
    </PageScaffold>
  );
}

interface MaterialButton {
  page: SubjectSection;
  icon: IconName;
  label: string;
  count: number;
}

/** A button for each kind of material the subject holds; Flashcards and Documents always show. */
function materialButtons(tile: SubjectTile): MaterialButton[] {
  const { summary } = tile;
  const buttons: MaterialButton[] = [
    { page: "decks", icon: "cards", label: "Flashcards", count: summary.cardCount },
    { page: "study-guides", icon: "study-guide", label: summary.guideCount === 1 ? "Guide" : "Guides", count: summary.guideCount },
    { page: "lessons", icon: "monitor", label: "Lessons", count: summary.lessonCount },
    { page: "podcasts", icon: "headphones", label: summary.podcastCount === 1 ? "Podcast" : "Podcasts", count: summary.podcastCount },
    { page: "documents", icon: "document", label: "Documents", count: summary.sourceCount },
  ];
  return buttons.filter((button) => button.page === "decks" || button.page === "documents" || button.count > 0);
}

function tileMeta(tile: SubjectTile): string {
  const { summary } = tile;
  const parts = [
    countLabel(summary.cardCount, "card"),
    summary.guideCount > 0 ? countLabel(summary.guideCount, "guide") : null,
    summary.lessonCount > 0 ? countLabel(summary.lessonCount, "lesson") : null,
    summary.podcastCount > 0 ? countLabel(summary.podcastCount, "podcast") : null,
    countLabel(summary.sourceCount, "document"),
  ];
  return parts.filter(Boolean).join(" · ");
}

interface LibrarySubjectTileProps {
  tile: SubjectTile;
  onOpen: (page: SubjectSection) => void;
  actions: React.ReactNode;
}

/** One subject: its figure, name, what it holds or is preparing, and its pages. */
export function LibrarySubjectTile({ tile, onOpen, actions }: LibrarySubjectTileProps) {
  const metaId = useId();
  const color = tile.colorIndex === null ? null : SUBJECT_COLORS[tile.colorIndex];
  return (
    <article
      data-testid="library-subject-tile"
      data-subject-id={tile.subjectId ?? "none"}
      className="tw:flex tw:h-full tw:flex-col tw:overflow-hidden tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:compact:flex-row tw:compact:flex-wrap"
    >
      <ItemTrigger
        tabIndex={-1}
        aria-hidden="true"
        onClick={() => onOpen("decks")}
        className="tw:relative tw:block tw:h-36 tw:w-full tw:cursor-pointer tw:overflow-hidden tw:border-0 tw:border-b tw:border-solid tw:border-border tw:p-0 tw:compact:m-3 tw:compact:mr-0 tw:compact:size-18 tw:compact:rounded-md tw:compact:border-b-0"
      >
        {tile.summary.cover ? (
          <CoverFigure cover={tile.summary.cover} />
        ) : (
          <span
            className={cn(
              "tw:grid tw:size-full tw:place-items-center tw:font-heading tw:text-[2.5rem] tw:font-semibold tw:compact:text-title-small",
              color ? color.soft : "tw:bg-(--ostudy-color-canvas-inset) tw:text-muted-foreground",
            )}
          >
            {tile.subjectId === null ? <Icon name="folder" size={36} /> : tile.initials}
          </span>
        )}
        {tile.dueCount > 0 ? (
          <span className="tw:absolute tw:top-2.5 tw:right-2.5 tw:rounded-full tw:bg-background tw:px-2.5 tw:py-0.5 tw:text-label tw:font-semibold tw:text-foreground tw:compact:hidden">
            {tile.dueCount.toLocaleString()} due
          </span>
        ) : null}
      </ItemTrigger>
      <div className="tw:flex tw:min-w-0 tw:flex-1 tw:flex-col tw:gap-2.5 tw:p-4 tw:compact:p-3">
        <div className="tw:flex tw:items-start tw:gap-2">
          <h2 className="tw:m-0 tw:min-w-0 tw:flex-1 tw:text-body tw:leading-snug tw:font-semibold tw:wrap-anywhere">
            <ItemTrigger
                data-testid="library-subject-open"
              aria-describedby={metaId}
              onClick={() => onOpen("decks")}
              className="tw:cursor-pointer tw:border-0 tw:bg-transparent tw:p-0 tw:text-start tw:font-sans tw:text-inherit tw:font-semibold tw:text-foreground tw:outline-none tw:hover:underline tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50"
            >
              {tile.name}
            </ItemTrigger>
          </h2>
          {tile.dueCount > 0 ? (
            <span className="tw:hidden tw:shrink-0 tw:rounded-full tw:bg-accent tw:px-2 tw:py-0.5 tw:text-label tw:font-semibold tw:text-accent-foreground tw:compact:inline">
              {tile.dueCount.toLocaleString()} due
            </span>
          ) : null}
          {actions}
        </div>
        <p id={metaId} className="tw:m-0 tw:flex tw:items-center tw:gap-1.5 tw:text-small tw:text-muted-foreground">
          {tile.preparing ? <Icon name="sparkles" size={14} className="tw:shrink-0" /> : null}
          <span className="tw:min-w-0 tw:truncate">
            {tile.preparing ? `${tile.preparing.step}: ${tile.preparing.documentTitle}` : tileMeta(tile)}
          </span>
        </p>
      </div>
      <div className="tw:mt-auto tw:flex tw:flex-wrap tw:gap-1.5 tw:px-4 tw:pb-4 tw:compact:w-full tw:compact:px-3 tw:compact:pb-3">
        {materialButtons(tile).map((button) => (
          <Button
            key={button.page}
            size="small"
            data-testid={`library-subject-${button.page}`}
            aria-label={`${tile.name}: ${button.label}`}
            onClick={() => onOpen(button.page)}
            className="tw:compact:flex-1"
          >
            <Icon name={button.icon} size={15} /> {button.label}
          </Button>
        ))}
      </div>
    </article>
  );
}

/** Drawn figures by tile, kept while the app runs so Library opens without redrawing them. */
const drawnCovers = new Map<string, string>();

/** The subject's figure, drawn from its page. */
function CoverFigure({ cover }: { cover: SubjectCover }) {
  const tile = cropTile(cover.pageNumber, cover.pageWidth, cover.pageHeight, cover);
  const key = `${cover.documentId}:${JSON.stringify(tile)}`;
  const [url, setUrl] = useState<string | null>(() => drawnCovers.get(key) ?? null);
  useEffect(() => {
    const drawn = drawnCovers.get(key);
    if (drawn) {
      setUrl(drawn);
      return;
    }
    let current = true;
    renderDocumentPageTile(cover.documentId, tile)
      .then((bytes) => {
        const objectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        drawnCovers.set(key, objectUrl);
        if (current) setUrl(objectUrl);
      })
      .catch(() => undefined);
    return () => {
      current = false;
    };
    // The tile is rebuilt every render; its key says when it changed.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return (
    <span className="tw:grid tw:size-full tw:place-items-center tw:bg-[var(--ostudy-color-neutral-0)]">
      {url ? <img src={url} alt="" className="tw:block tw:size-full tw:object-contain tw:p-2" /> : null}
    </span>
  );
}

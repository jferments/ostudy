import { useId, useState, type FormEvent } from "react";
import type { ShowToast } from "../components/Toast";
import type { StudyGuidePageItem, StudyGuidePdfTheme, StudyGuideLibrarySort, StudyGuideSubjectFilter } from "../ipc/studyGuides";
import { getErrorMessage } from "../ipc/common";
import { openDocumentSource } from "../ipc/documents";
import { PerformanceProfiler } from "../testing/PerformanceProfiler";
import { Icon } from "../ui/icons/Icon";
import { Button } from "../ui/primitives/Button";
import { TextField } from "../ui/primitives/TextField";
import { NativeSelect } from "../ui/primitives/NativeSelect";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { Progress } from "../ui/primitives/Progress";
import { EmptyState, LoadingState } from "../ui/primitives/EmptyState";
import { useConfirm } from "../ui/overlays/overlayContext";
import { OverflowMenu, type OverflowMenuItem } from "../ui/overlays/OverflowMenu";
import { cn } from "../ui/internal/cn";
import { PageHeader, PageScaffold } from "../ui/patterns/WorkspacePatterns";
import { deleteTargetFromItem, guideSourceFromItem, useStudyGuidesController, type StudyGuideDeleteTarget, type StudyGuideOpenRequest } from "../controllers/useStudyGuidesController";
import { StudyGuideDialog } from "./StudyGuideDialog";
import { CollectionStudyGuideDialog } from "./CollectionStudyGuideDialog";
import { guideLibraryStyles as styles } from "./documents/presentation";
import { useLibraryFullPage } from "./library/LibraryFrame";
import { useGenerationModelMissing } from "./documents/useGenerationModelMissing";
import { exportGuidePdf } from "./study-guides/exportGuidePdf";
import { useGuideRemake } from "./study-guides/useGuideRemake";
import { useOptionalTutor } from "./tutor/TutorContext";

interface StudyGuidesScreenProps {
  /** Lists one subject's guides; every guide when left out. */
  subject?: StudyGuideSubjectFilter;
  showToast: ShowToast;
  onPracticeCards: (cardIds: string[]) => Promise<void>;
  openRequest?: StudyGuideOpenRequest | null;
  onOpenRequestConsumed?: (requestId: number) => void;
  /** Opens AI setup; Remake goes there while no model is chosen for guides. */
  onSetUpAi?: () => void;
  /** Where links on a guide's pages lead: a deck's Practice, and a notes page. */
  onPracticeDeck?: (deckId: string) => void | Promise<void>;
  onOpenNotebook?: (notebookId: string, pageId: string) => void;
  /** The study guide theme from Settings, which the reader shows and can change. */
  guideTheme?: StudyGuidePdfTheme;
  onGuideThemeChange?: (theme: StudyGuidePdfTheme) => void;
}

export function StudyGuidesScreen({ subject, showToast, onPracticeCards, openRequest = null, onOpenRequestConsumed, onSetUpAi, onPracticeDeck, onOpenNotebook, guideTheme = "sage", onGuideThemeChange }: StudyGuidesScreenProps) {
  const confirm = useConfirm();
  const controller = useStudyGuidesController({ subject, showToast, openRequest, onOpenRequestConsumed });
  const tutor = useOptionalTutor();
  const remaking = useGuideRemake({ showToast, onRemade: controller.refresh });
  const modelMissing = useGenerationModelMissing();
  const [exportingGuideId, setExportingGuideId] = useState<string | null>(null);
  // An open guide is a page of its own in Library, without the view switcher.
  useLibraryFullPage(controller.selected !== null);
  async function confirmDelete(target: StudyGuideDeleteTarget): Promise<boolean> {
    const accepted = await confirm({
      title: `Delete “${target.documentTitle}” study guide?`,
      description: "This removes every generated guide revision, guide-specific section edits and conflicts, saved generation checkpoints, and prepared PDF exports. The source document, its learning plan, linked flashcards, and your stable guide/topic notes are preserved. You can generate a new guide later.",
      confirmLabel: "Delete study guide", destructive: true,
    });
    return accepted ? controller.deleteGuide(target) : false;
  }
  async function openSource(documentId: string): Promise<void> {
    try { await openDocumentSource(documentId); } catch (error: unknown) { showToast("error", getErrorMessage(error)); }
  }
  async function exportPdf(item: StudyGuidePageItem): Promise<void> {
    if (exportingGuideId !== null) return;
    setExportingGuideId(item.guideId);
    try { await exportGuidePdf({ kind: item.targetKind, id: item.targetId }, confirm, showToast); }
    finally { setExportingGuideId(null); }
  }
  /** What the card's menu offers beyond Open: the reader's own actions, then the source and Delete. */
  function cardMenu(item: StudyGuidePageItem): OverflowMenuItem[] {
    const document = item.targetKind === "document";
    const remakeRunning = document && remaking.running(item.targetId);
    const deleteTarget = deleteTargetFromItem(item);
    const deleting = controller.deletingGuideId === item.guideId;
    return [
      ...(tutor ? [{
        id: "tutor", label: "Ask Tutor", disabled: tutor.busy || tutor.pending !== null,
        onSelect: () => tutor.openContext(document
          ? { title: item.title, scope: { kind: "guide", id: item.targetId },
            focus: { kind: "guide", documentId: item.targetId, guideId: item.guideId, topicId: null, contentHash: null }, mode: "explain" }
          : { title: item.title, scope: { kind: "collection", id: item.targetId }, focus: null, mode: "explain" }),
      } satisfies OverflowMenuItem] : []),
      { id: "export", label: exportingGuideId === item.guideId ? "Exporting PDF…" : "Export PDF", disabled: exportingGuideId !== null || deleting,
        onSelect: () => void exportPdf(item) },
      ...(!document ? [] : remakeRunning ? [{
        id: "stop-remake", label: remaking.stopping(item.targetId) ? "Stopping…" : "Stop remaking",
        disabled: remaking.stopping(item.targetId), onSelect: () => void remaking.stop(item.targetId),
      }] : [{
        id: "remake", label: "Remake the study guide", disabled: deleting,
        onSelect: modelMissing && onSetUpAi ? onSetUpAi : () => void remaking.remake(item.targetId),
      }]),
      ...(document ? [{ id: "source", label: "Open the source", onSelect: () => void openSource(item.targetId) }] : []),
      ...(deleteTarget ? [{ id: "delete", label: "Delete study guide", destructive: true, disabled: deleting || remakeRunning,
        onSelect: () => void confirmDelete(deleteTarget) }] : []),
    ];
  }
  function submitSearch(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    controller.submitSearch();
  }
  // The document reader is a whole page with its own sticky top bar, so it starts at the edges.
  if (controller.selected) return (
    <PageScaffold width="full" inset={controller.selected.targetKind === "collection"} aria-label="Study Guides">
      <PerformanceProfiler id="guide-dialog">
        {controller.selected.targetKind === "collection" ? (
          <CollectionStudyGuideDialog
            collectionId={controller.selected.targetId}
            collectionTitle={controller.selected.title}
            showToast={showToast}
            onClose={controller.closeGuide}
          />
        ) : (
          <StudyGuideDialog
            key={controller.selected.targetId}
            documentId={controller.selected.targetId}
            documentTitle={controller.selected.title}
            source={controller.selected.source ?? null}
            showToast={showToast}
            theme={guideTheme}
            onThemeChange={onGuideThemeChange}
            onPracticeCards={onPracticeCards}
            onPracticeDeck={onPracticeDeck}
            onOpenNotebook={onOpenNotebook}
            onOpenGuide={(documentId, title) => controller.openGuide({ targetKind: "document", targetId: documentId, title })}
            onClose={controller.closeGuide}
          />
        )}
      </PerformanceProfiler>
    </PageScaffold>
  );
  return (
    <PageScaffold width="full" aria-label="Study Guides">
      <PageHeader title="Study Guides" compact actions={
        <form className={styles.toolbar} role="search" onSubmit={submitSearch}>
          <label className={styles.search}>
            <span className="sr-only">Search guides</span>
            <Icon name="search" size={16} className="tw:pointer-events-none tw:absolute tw:left-3 tw:text-muted-foreground" />
            <TextField type="search" value={controller.draftQuery} placeholder="Search guides" className="tw:w-full tw:pl-9"
              onChange={(event) => controller.setDraftQuery(event.currentTarget.value)} />
          </label>
          <label className="tw:flex tw:items-center tw:gap-2 tw:text-small tw:text-muted-foreground">
            <span className="tw:compact:sr-only">Sort</span>
            <NativeSelect value={controller.sort} onChange={(event) => controller.setSort(event.currentTarget.value as StudyGuideLibrarySort)}>
              <option value="recent">Newest</option><option value="oldest">Oldest</option>
              <option value="title_asc">Title A–Z</option><option value="title_desc">Title Z–A</option>
            </NativeSelect>
          </label>
        </form>
      } />
      <PerformanceProfiler id="guide-library">
        <section aria-label="Study guides" className={styles.workspace}>
          {controller.query ? (
            <p className={styles.summary} aria-live="polite">
              {controller.page.totalCount} guide{controller.page.totalCount === 1 ? "" : "s"} matching “{controller.query}”
            </p>
          ) : null}
          {controller.errorMessage ? <InlineNotice kind="error" title="Study Guides could not be refreshed." className="tw:mb-4">{controller.errorMessage}</InlineNotice> : null}
          {controller.isLoading && controller.page.items.length === 0 ? <LoadingState label="Loading study guides…" /> : controller.page.items.length === 0 ? (
            <EmptyState icon={<Icon name="study-guide" size={30} />} title={controller.query ? "No matching study guides" : "No study guides yet"}
              description={controller.query ? undefined : "Make guides from Documents."} />
          ) : (
            <ul aria-label="Study guides" className={styles.grid}>
              {controller.page.items.map((item) => (
                <GuideCard
                  key={item.guideId}
                  item={item}
                  deleting={controller.deletingGuideId === item.guideId}
                  remakeProgress={item.targetKind === "document" ? remaking.progressOf(item.targetId) : null}
                  menu={cardMenu(item)}
                  onOpen={() => controller.openGuide({
                    targetKind: item.targetKind,
                    targetId: item.targetId,
                    title: item.title,
                    source: guideSourceFromItem(item),
                  })}
                />
              ))}
            </ul>
          )}
          {controller.page.totalCount > controller.page.limit ? (
            <nav className={styles.pagination} aria-label="Study Guides pages">
              <Button disabled={!controller.canGoPrevious || controller.isLoading} onClick={controller.goPrevious}>Previous</Button>
              <span>Page {controller.pageNumber} of {controller.pageCount}</span>
              <Button disabled={!controller.canGoNext || controller.isLoading} onClick={controller.goNext}>Next</Button>
            </nav>
          ) : null}
        </section>
      </PerformanceProfiler>
    </PageScaffold>
  );
}

function GuideCard({
  item,
  deleting,
  remakeProgress,
  menu,
  onOpen,
}: {
  item: StudyGuidePageItem;
  deleting: boolean;
  /** Set while the guide is being remade; the old guide stays open to read. */
  remakeProgress: { label: string; value: number | null } | null;
  menu: OverflowMenuItem[];
  onOpen: () => void;
}) {
  const headingId = `guide-${useId().replaceAll(":", "")}`;
  const collection = item.targetKind === "collection";
  return (
    <li data-testid="guide-card" className={styles.card}>
      <article aria-labelledby={headingId} className="tw:relative tw:h-full tw:min-w-0">
        {/* A subject's guide covers several documents, so it sits on a stack. */}
        {collection ? <span aria-hidden="true" className={styles.stack} /> : null}
        <div className={cn(styles.cardSurface, collection && styles.collectionSurface)}>
          <div className="tw:flex tw:min-w-0 tw:items-start tw:gap-3">
            <span aria-hidden="true" className={styles.cover}><Icon name={collection ? "folder" : "study-guide"} size={20} /></span>
            <div className="tw:grid tw:min-w-0 tw:flex-1 tw:gap-0.5">
              <h3 id={headingId} className="tw:m-0 tw:wrap-anywhere tw:text-body tw:font-semibold tw:leading-snug">{item.title}</h3>
              <span className="tw:text-small tw:text-muted-foreground">{sourceSummary(item)}</span>
            </div>
            {menu.length ? <OverflowMenu label={`More actions for ${item.title}`} items={menu} /> : null}
          </div>
          {remakeProgress ? (
            <Progress value={remakeProgress.value} max={100} size="small" label={`${item.title} remake progress`}
              valueText={remakeProgress.label} text={<span className="tw:tabular-nums">{remakeProgress.label}</span>} />
          ) : collection ? null : <ReadingProgress item={item} />}
          <div className="tw:mt-auto tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:justify-between tw:gap-x-3 tw:gap-y-2">
            <span className={styles.metrics}>
              <span>{item.sectionCount} section{item.sectionCount === 1 ? "" : "s"}</span>
              {item.noteCount > 0 ? <span>{item.noteCount} note{item.noteCount === 1 ? "" : "s"}</span> : null}
              <span>{formatDate(item.publishedAt)}</span>
            </span>
            <Button variant="secondary" size="small" disabled={deleting} onClick={onOpen}>Open</Button>
          </div>
        </div>
      </article>
    </li>
  );
}
/** How much of a document guide is read; a guide not opened yet says so. */
function ReadingProgress({ item }: { item: StudyGuidePageItem }) {
  const read = Math.min(item.readSectionCount, item.sectionCount);
  const finished = item.sectionCount > 0 && read === item.sectionCount;
  const status = finished ? "Finished" : read === 0 && item.lastOpenedAt === null ? "Not started"
    : `${read} of ${item.sectionCount} section${item.sectionCount === 1 ? "" : "s"} read`;
  return (
    <Progress value={read} max={Math.max(1, item.sectionCount)} size="small" tone={finished ? "success" : "accent"}
      label={`${item.title} reading progress`} valueText={status}
      text={<span className={cn("tw:tabular-nums", finished && "tw:font-semibold tw:text-success")}>{status}</span>} />
  );
}

function sourceSummary(item: StudyGuidePageItem): string {
  if (item.targetKind === "collection") {
    return `Collection · ${item.sourceCount.toLocaleString()} document${item.sourceCount === 1 ? "" : "s"}`;
  }
  const format = (item.sourceFormat ?? "document").replaceAll("_", " ").toUpperCase();
  const count = item.sourceUnitCount ?? 0;
  const kind = item.sourceUnitKind ?? "page";
  const unit = count === 1 ? singularUnit(kind) : pluralUnit(kind);
  return count > 0 ? `${format} · ${count.toLocaleString()} ${unit}` : format;
}
function singularUnit(kind: string): string {
  switch (kind) {
    case "slide": return "slide";
    case "chapter": return "chapter";
    case "section": return "section";
    case "sheet": return "sheet";
    case "row_group": return "row group";
    default: return "page";
  }
}
function pluralUnit(kind: string): string {
  const singular = singularUnit(kind);
  return singular === "row group" ? "row groups" : `${singular}s`;
}
function formatDate(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(timestamp));
}

import type { FormEvent } from "react";
import type { ShowToast } from "../components/Toast";
import type { StudyGuidePageItem, StudyGuideLibrarySort } from "../ipc/studyGuides";
import { PerformanceProfiler } from "../testing/PerformanceProfiler";
import { Icon } from "../ui/icons/Icon";
import { Button } from "../ui/primitives/Button";
import { Surface } from "../ui/primitives/Surface";
import { TextField } from "../ui/primitives/TextField";
import { NativeSelect } from "../ui/primitives/NativeSelect";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { ListRow, ListRowMedia, ListRows } from "../ui/primitives/ListRow";
import { EmptyState, LoadingState } from "../ui/primitives/EmptyState";
import { useConfirm } from "../ui/overlays/overlayContext";
import { ResponsiveActionGroup } from "../ui/patterns/ResponsiveActionGroup";
import { PageHeader, PageScaffold } from "../ui/patterns/WorkspacePatterns";
import { deleteTargetFromItem, guideSourceFromItem, useStudyGuidesController, type StudyGuideDeleteTarget, type StudyGuideOpenRequest } from "../controllers/useStudyGuidesController";
import { StudyGuideDialog } from "./StudyGuideDialog";
import { CollectionStudyGuideDialog } from "./CollectionStudyGuideDialog";
import { guideLibraryStyles as styles } from "./documents/presentation";

interface StudyGuidesScreenProps {
  showToast: ShowToast;
  onPracticeCards: (cardIds: string[]) => Promise<void>;
  openRequest?: StudyGuideOpenRequest | null;
  onOpenRequestConsumed?: (requestId: number) => void;
}

export function StudyGuidesScreen({ showToast, onPracticeCards, openRequest = null, onOpenRequestConsumed }: StudyGuidesScreenProps) {
  const confirm = useConfirm();
  const controller = useStudyGuidesController({ showToast, openRequest, onOpenRequestConsumed });
  async function confirmDelete(target: StudyGuideDeleteTarget): Promise<boolean> {
    const accepted = await confirm({
      title: `Delete “${target.documentTitle}” study guide?`,
      description: "This removes every generated guide revision, guide-specific section edits and conflicts, saved generation checkpoints, and prepared PDF exports. The source document, its learning plan, linked flashcards, and your stable guide/topic notes are preserved. You can generate a new guide later.",
      confirmLabel: "Delete study guide", destructive: true,
    });
    return accepted ? controller.deleteGuide(target) : false;
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
            documentId={controller.selected.targetId}
            documentTitle={controller.selected.title}
            source={controller.selected.source ?? null}
            showToast={showToast}
            onPracticeCards={onPracticeCards}
            onClose={controller.closeGuide}
          />
        )}
      </PerformanceProfiler>
    </PageScaffold>
  );
  return (
    <PageScaffold width="full" aria-label="Study Guides">
      <PageHeader title="Study Guides" icon="study-guide" description="Open source-grounded guides, annotate important topics, and practice linked flashcards." />
      <PerformanceProfiler id="guide-library">
        <Surface as="section" level={2} className={styles.workspace}>
          <div className={styles.toolbar}>
            <form className={styles.search} role="search" onSubmit={submitSearch}>
              <label><span>Search guides</span>
                <TextField type="search" value={controller.draftQuery} placeholder="Title, filename, or format"
                  onChange={(event) => controller.setDraftQuery(event.currentTarget.value)} />
              </label>
              <Button type="submit" variant="secondary">Search</Button>
              {controller.query ? <Button variant="ghost" onClick={controller.clearSearch}>Clear</Button> : null}
            </form>
            <div className={styles.tools}>
              <label><span>Sort</span>
                <NativeSelect value={controller.sort} onChange={(event) => controller.setSort(event.currentTarget.value as StudyGuideLibrarySort)}>
                  <option value="recent">Newest</option><option value="oldest">Oldest</option>
                  <option value="title_asc">Title A–Z</option><option value="title_desc">Title Z–A</option>
                </NativeSelect>
              </label>
              <Button disabled={controller.isLoading} loading={controller.isRefreshing} onClick={() => void controller.refresh()}>Refresh</Button>
            </div>
          </div>
          <div className={styles.summary} aria-live="polite">
            <span>{controller.page.totalCount} guide{controller.page.totalCount === 1 ? "" : "s"}{controller.query ? ` matching “${controller.query}”` : ""}</span>
            {controller.visibleRange ? <span>Showing {controller.visibleRange.first}–{controller.visibleRange.last}</span> : null}
          </div>
          {controller.errorMessage ? <InlineNotice kind="error" title="Study Guides could not be refreshed." className="tw:mb-4">{controller.errorMessage}</InlineNotice> : null}
          {controller.isLoading && controller.page.items.length === 0 ? <LoadingState label="Loading study guides…" /> : controller.page.items.length === 0 ? (
            <EmptyState icon={<Icon name="study-guide" size={30} />} title={controller.query ? "No matching study guides" : "No study guides yet"}
              description={controller.query ? "Change the search text or clear the filter." : "Generate a guide from Documents. It will appear here without loading every guide body."} />
          ) : (
            <ListRows as="div">
              {controller.page.items.map((item) => {
                const deleteTarget = deleteTargetFromItem(item);
                return <GuideRow
                  key={item.guideId}
                  item={item}
                  deleting={controller.deletingGuideId === item.guideId}
                  onOpen={() => controller.openGuide({
                    targetKind: item.targetKind,
                    targetId: item.targetId,
                    title: item.title,
                    source: guideSourceFromItem(item),
                  })}
                  onDelete={deleteTarget ? () => void confirmDelete(deleteTarget) : null}
                />;
              })}
            </ListRows>
          )}
          {controller.page.totalCount > controller.page.limit ? (
            <nav className={styles.pagination} aria-label="Study Guides pages">
              <Button disabled={!controller.canGoPrevious || controller.isLoading} onClick={controller.goPrevious}>Previous</Button>
              <span>Page {controller.pageNumber} of {controller.pageCount}</span>
              <Button disabled={!controller.canGoNext || controller.isLoading} onClick={controller.goNext}>Next</Button>
            </nav>
          ) : null}
        </Surface>
      </PerformanceProfiler>
    </PageScaffold>
  );
}

function GuideRow({
  item,
  deleting,
  onOpen,
  onDelete,
}: {
  item: StudyGuidePageItem;
  deleting: boolean;
  onOpen: () => void;
  onDelete: (() => void) | null;
}) {
  const actions = [
    { id: "open", label: "Open", placement: "always-visible" as const, variant: "primary" as const, disabled: deleting, onSelect: onOpen },
    ...(onDelete ? [{
      id: "delete",
      label: `Delete ${item.title} study guide`,
      description: "Delete generated guide revisions while preserving the source, flashcards, and stable notes.",
      placement: "wide-and-medium" as const,
      presentation: "icon" as const,
      icon: "delete" as const,
      destructive: true,
      disabled: deleting,
      loading: deleting,
      onSelect: onDelete,
    }] : []),
  ];
  return <ListRow as="article" leading={<ListRowMedia icon="study-guide" />} title={item.title} onActivate={onOpen} activateDisabled={deleting}
    meta={<>
      <span className="tw:wrap-anywhere">{sourceSummary(item)}</span>
      <span className={styles.metrics}>
        <span>{item.sectionCount} section{item.sectionCount === 1 ? "" : "s"}</span>
        <span>{item.noteCount} note{item.noteCount === 1 ? "" : "s"}</span><span>{formatDate(item.publishedAt)}</span>
      </span>
    </>}
    trailing={<ResponsiveActionGroup
      label={`Actions for ${item.title}`}
      overflowLabel={`${item.title} actions`}
      actions={actions}
    />} />;
}
function sourceSummary(item: StudyGuidePageItem): string {
  if (item.targetKind === "collection") {
    return `Document collection · ${item.sourceCount.toLocaleString()} source document${item.sourceCount === 1 ? "" : "s"}`;
  }
  const format = (item.sourceFormat ?? "document").replaceAll("_", " ").toUpperCase();
  const count = item.sourceUnitCount ?? 0;
  const kind = item.sourceUnitKind ?? "page";
  const unit = count === 1 ? singularUnit(kind) : pluralUnit(kind);
  return `${item.originalFilename ?? "Document"} · ${format} · ${count.toLocaleString()} ${unit}`;
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
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(timestamp));
}

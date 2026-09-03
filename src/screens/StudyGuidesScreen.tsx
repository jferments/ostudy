import type { FormEvent } from "react";

import type { ShowToast } from "../components/Toast";
import type { StudyGuidePageItem, StudyGuideLibrarySort } from "../ipc/studyGuides";
import { PerformanceProfiler } from "../testing/PerformanceProfiler";
import { Icon } from "../ui/icons/Icon";
import { Button } from "../ui/primitives/Button";
import { Surface } from "../ui/primitives/Surface";
import { useConfirm } from "../ui/overlays/overlayContext";
import { ResponsiveActionGroup } from "../ui/patterns/ResponsiveActionGroup";
import { PageHeader, PageScaffold } from "../ui/patterns/WorkspacePatterns";
import {
  deleteTargetFromItem,
  useStudyGuidesController,
  type StudyGuideDeleteTarget,
  type StudyGuideOpenRequest,
} from "../controllers/useStudyGuidesController";
import { StudyGuideDialog } from "./StudyGuideDialog";
import styles from "./StudyGuidesScreen.module.css";

interface StudyGuidesScreenProps {
  showToast: ShowToast;
  onPracticeCards: (cardIds: string[]) => Promise<void>;
  openRequest?: StudyGuideOpenRequest | null;
  onOpenRequestConsumed?: (requestId: number) => void;
}

export function StudyGuidesScreen({
  showToast,
  onPracticeCards,
  openRequest = null,
  onOpenRequestConsumed,
}: StudyGuidesScreenProps) {
  const confirm = useConfirm();
  const controller = useStudyGuidesController({
    showToast,
    openRequest,
    onOpenRequestConsumed,
  });

  async function confirmDelete(target: StudyGuideDeleteTarget): Promise<boolean> {
    const accepted = await confirm({
      title: `Delete “${target.documentTitle}” study guide?`,
      description:
        "This removes every generated guide revision, guide-specific section edits and conflicts, saved generation checkpoints, and prepared PDF exports. The source document, its learning plan, linked flashcards, and your stable guide/topic notes are preserved. You can generate a new guide later.",
      confirmLabel: "Delete study guide",
      destructive: true,
    });
    return accepted ? controller.deleteGuide(target) : false;
  }

  function submitSearch(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    controller.submitSearch();
  }

  if (controller.selected) {
    return (
      <PageScaffold width="full" aria-label="Study Guides">
        <PerformanceProfiler id="guide-dialog">
          <StudyGuideDialog
            documentId={controller.selected.documentId}
            documentTitle={controller.selected.documentTitle}
            showToast={showToast}
            onPracticeCards={onPracticeCards}
            onClose={controller.closeGuide}
          />
        </PerformanceProfiler>
      </PageScaffold>
    );
  }

  return (
    <PageScaffold width="full" aria-label="Study Guides">
      <PageHeader
        title="Study Guides"
        icon="study-guide"
        description="Open source-grounded guides, annotate important topics, and practice linked flashcards."
        compact
      />

      <PerformanceProfiler id="guide-library">
        <Surface as="section" level={2} className={styles.workspace}>
          <div className={styles.toolbar}>
            <form className={styles.search} role="search" onSubmit={submitSearch}>
              <label>
                <span>Search guides</span>
                <input
                  type="search"
                  value={controller.draftQuery}
                  placeholder="Title, filename, or format"
                  onChange={(event) => controller.setDraftQuery(event.currentTarget.value)}
                />
              </label>
              <Button type="submit" variant="secondary">
                Search
              </Button>
              {controller.query ? (
                <Button type="button" variant="ghost" onClick={controller.clearSearch}>
                  Clear
                </Button>
              ) : null}
            </form>

            <div className={styles.tools}>
              <label>
                <span>Sort</span>
                <select
                  value={controller.sort}
                  onChange={(event) =>
                    controller.setSort(event.currentTarget.value as StudyGuideLibrarySort)
                  }
                >
                  <option value="recent">Newest</option>
                  <option value="oldest">Oldest</option>
                  <option value="title_asc">Title A–Z</option>
                  <option value="title_desc">Title Z–A</option>
                </select>
              </label>
              <Button
                type="button"
                variant="secondary"
                disabled={controller.isLoading}
                loading={controller.isRefreshing}
                onClick={() => void controller.refresh()}
              >
                Refresh
              </Button>
            </div>
          </div>

          <div className={styles.summary} aria-live="polite">
            <span>
              {controller.page.totalCount} guide
              {controller.page.totalCount === 1 ? "" : "s"}
              {controller.query ? ` matching “${controller.query}”` : ""}
            </span>
            {controller.visibleRange ? (
              <span>
                Showing {controller.visibleRange.first}–{controller.visibleRange.last}
              </span>
            ) : null}
          </div>

          {controller.errorMessage ? (
            <div className={styles.error} role="alert">
              <strong>Study Guides could not be refreshed.</strong>
              <span>{controller.errorMessage}</span>
            </div>
          ) : null}

          {controller.isLoading && controller.page.items.length === 0 ? (
            <div className={styles.state} aria-live="polite">
              Loading study guides…
            </div>
          ) : controller.page.items.length === 0 ? (
            <div className={styles.empty}>
              <Icon name="study-guide" size={30} />
              <strong>{controller.query ? "No matching study guides" : "No study guides yet"}</strong>
              <span>
                {controller.query
                  ? "Change the search text or clear the filter."
                  : "Generate a guide from Documents. It will appear here without loading every guide body."}
              </span>
            </div>
          ) : (
            <div className={styles.list}>
              {controller.page.items.map((item) => (
                <GuideRow
                  key={item.guideId}
                  item={item}
                  deleting={controller.deletingGuideId === item.guideId}
                  onOpen={() => controller.openGuide(item)}
                  onDelete={() => void confirmDelete(deleteTargetFromItem(item))}
                />
              ))}
            </div>
          )}

          {controller.page.totalCount > controller.page.limit ? (
            <nav className={styles.pagination} aria-label="Study Guides pages">
              <Button
                type="button"
                variant="secondary"
                disabled={!controller.canGoPrevious || controller.isLoading}
                onClick={controller.goPrevious}
              >
                Previous
              </Button>
              <span>
                Page {controller.pageNumber} of {controller.pageCount}
              </span>
              <Button
                type="button"
                variant="secondary"
                disabled={!controller.canGoNext || controller.isLoading}
                onClick={controller.goNext}
              >
                Next
              </Button>
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
  onDelete: () => void;
}) {
  return (
    <article className={styles.row}>
      <button type="button" className={styles.rowMain} onClick={onOpen}>
        <span className={styles.rowIcon} aria-hidden="true">
          <Icon name="study-guide" size={22} />
        </span>
        <span className={styles.rowCopy}>
          <strong>{item.documentTitle}</strong>
          <small>{sourceSummary(item)}</small>
          <span className={styles.metrics}>
            <span>
              {item.sectionCount} section{item.sectionCount === 1 ? "" : "s"}
            </span>
            <span>
              {item.noteCount} note{item.noteCount === 1 ? "" : "s"}
            </span>
            <span>Revision {item.guideRevision}</span>
            <span>{formatDate(item.publishedAt)}</span>
          </span>
        </span>
      </button>
      <ResponsiveActionGroup
        label={`Actions for ${item.documentTitle}`}
        overflowLabel={`${item.documentTitle} actions`}
        actions={[
          {
            id: "open",
            label: "Open",
            placement: "always-visible",
            variant: "primary",
            disabled: deleting,
            onSelect: onOpen,
          },
          {
            id: "delete",
            label: `Delete ${item.documentTitle} study guide`,
            description:
              "Delete generated guide revisions while preserving the source, flashcards, and stable notes.",
            placement: "wide-and-medium",
            presentation: "icon",
            icon: "delete",
            destructive: true,
            disabled: deleting,
            loading: deleting,
            onSelect: onDelete,
          },
        ]}
        className={styles.actions}
      />
    </article>
  );
}

function sourceSummary(item: StudyGuidePageItem): string {
  const format = item.sourceFormat.replaceAll("_", " ").toUpperCase();
  const unit =
    item.sourceUnitCount === 1
      ? singularUnit(item.sourceUnitKind)
      : pluralUnit(item.sourceUnitKind);
  return `${item.originalFilename} · ${format} · ${item.sourceUnitCount.toLocaleString()} ${unit}`;
}

function singularUnit(kind: string): string {
  switch (kind) {
    case "slide":
      return "slide";
    case "chapter":
      return "chapter";
    case "section":
      return "section";
    case "sheet":
      return "sheet";
    case "row_group":
      return "row group";
    default:
      return "page";
  }
}

function pluralUnit(kind: string): string {
  const singular = singularUnit(kind);
  return singular === "row group" ? "row groups" : `${singular}s`;
}

function formatDate(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(timestamp));
}

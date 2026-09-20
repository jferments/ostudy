import { useCallback, useEffect, useMemo, useState } from "react";

import { StudyMarkdown } from "../components/StudyMarkdown";
import type { ShowToast } from "../components/Toast";
import { getErrorMessage } from "../ipc/common";
import { publishPreparedExport, releasePreparedExport } from "../ipc/externalFiles";
import { openDocumentSource } from "../ipc/documents";
import {
  getCollectionStudyGuide,
  prepareCollectionStudyGuidePdf,
  type CollectionGuideSection,
  type CollectionStudyGuide,
  type CollectionStudyGuideOverlay,
} from "../ipc/studyMaterials";
import { TutorEntryButton } from "./tutor/TutorEntryButton";
import { useOptionalTutor } from "./tutor/TutorContext";
import { OverflowMenu, type OverflowMenuItem } from "../ui/overlays/OverflowMenu";
import { useConfirm } from "../ui/overlays/overlayContext";
import { InlineWorkspace } from "../ui/overlays/Dialog";
import { EmptyState, LoadingState } from "../ui/primitives/EmptyState";
import { Checkbox } from "../ui/primitives/FormControls";
import { Button } from "../ui/primitives/Button";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { guideReaderStyles as styles } from "./documents/presentation";
import { StudyGuideStructuredContent } from "./StudyGuideDialog";

interface CollectionStudyGuideDialogProps {
  collectionId: string;
  collectionTitle: string;
  showToast: ShowToast;
  onClose: () => void;
}

export function CollectionStudyGuideDialog({
  collectionId,
  collectionTitle,
  showToast,
  onClose,
}: CollectionStudyGuideDialogProps) {
  const confirm = useConfirm();
  const [guide, setGuide] = useState<CollectionStudyGuide | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isExporting, setIsExporting] = useState(false);
  const [includeAnnotations, setIncludeAnnotations] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const refresh = useCallback(async () => {
    const loaded = await getCollectionStudyGuide(collectionId);
    setGuide(loaded);
    setErrorMessage(null);
    return loaded;
  }, [collectionId]);

  useEffect(() => {
    let active = true;
    setIsLoading(true);
    void refresh()
      .catch((error: unknown) => { if (active) setErrorMessage(getErrorMessage(error)); })
      .finally(() => { if (active) setIsLoading(false); });
    return () => { active = false; };
  }, [refresh]);

  const guideNote = useMemo(
    () => guide?.overlays.find((overlay) =>
      overlay.blockKey === null
      && overlay.overlayKind === "guide_note"
      && overlay.state === "active") ?? null,
    [guide],
  );

  async function exportPdf(): Promise<void> {
    if (!guide || isExporting) return;
    setIsExporting(true);
    let preparedExportId: string | null = null;
    try {
      const prepared = await prepareCollectionStudyGuidePdf(collectionId, includeAnnotations);
      preparedExportId = prepared.preparedExport.id;
      if (prepared.mathSourceFallback && !await confirm({
        title: "Export math as source notation?",
        description: "Equations in this PDF will use readable LaTeX source rather than typeset formulas. The study guide in oStudy keeps its typeset math.",
        confirmLabel: "Export PDF",
        cancelLabel: "Cancel",
      })) return;
      const receipt = await publishPreparedExport(prepared.preparedExport.id);
      if (receipt) {
        const notes = prepared.annotationCount > 0
          ? ` and ${prepared.annotationCount} note${prepared.annotationCount === 1 ? "" : "s"}`
          : "";
        showToast("success", `Exported collection study guide with ${prepared.sectionCount} section${prepared.sectionCount === 1 ? "" : "s"}${notes}.`);
      }
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      if (preparedExportId) await releasePreparedExport(preparedExportId).catch(() => undefined);
      setIsExporting(false);
    }
  }

  async function openSource(section: CollectionGuideSection): Promise<void> {
    if (!section.sourceDocumentId) return;
    try {
      await openDocumentSource(section.sourceDocumentId);
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  return (
    <InlineWorkspace
      title={`Study guide · ${collectionTitle}`}
      description="Source-grounded sections composed from the collection’s authorized document guides."
      onClose={onClose}
      closeLabel="Close collection study guide"
      className={styles.workspace}
      dismissible={!isExporting}
      busy={isExporting}
    >
      <header className={styles.header}>
        <div>
          <p className={styles.eyebrow}>Collection learning artifact</p>
          <strong>
            {guide
              ? `${guide.content.sections.length} section${guide.content.sections.length === 1 ? "" : "s"}`
              : "Not generated yet"}
          </strong>
        </div>
        <div className={styles.headerActions}>
          {guide ? (
            <label className={styles.annotationToggle}>
              <Checkbox
                checked={includeAnnotations}
                disabled={isExporting}
                onCheckedChange={setIncludeAnnotations}
              />
              Include my notes in PDF
            </label>
          ) : null}
          {guide ? (
            <Button variant="secondary" disabled={isExporting} onClick={() => void exportPdf()}>
              {isExporting ? "Exporting…" : "Export PDF"}
            </Button>
          ) : null}
          {guide ? (
            <TutorEntryButton
              target={{
                title: collectionTitle,
                scope: { kind: "collection", id: collectionId },
                focus: null,
                mode: "explain",
              }}
            />
          ) : null}
        </div>
      </header>
      {errorMessage ? <InlineNotice kind="error" className="tw:mb-5">{errorMessage}</InlineNotice> : null}
      {isLoading ? (
        <LoadingState label="Loading collection study guide…" />
      ) : !guide ? (
        <EmptyState
          title="No collection study guide yet"
          description="Create or update this collection’s study materials from Documents."
        />
      ) : (
        <div className={styles.layout}>
          <nav className={styles.contents} aria-label="Collection study guide contents">
            <strong>Contents</strong>
            <a href="#collection-guide-overview">Overview</a>
            {guide.content.sections.map((section, index) => (
              <a key={section.blockKey} href={`#collection-guide-section-${index}`}>{section.title}</a>
            ))}
          </nav>
          <article className={styles.guide}>
            <section id="collection-guide-overview" className={styles.overview}>
              <h2>Overview</h2>
              <StudyMarkdown text={guide.content.overview.summary} />
              <CollectionGuideList
                title="Learning objectives"
                values={guide.content.overview.learningObjectives}
              />
              <CollectionGuideList title="Key concepts" values={guide.content.overview.keyConcepts} />
              {guideNote?.plainText ? (
                <aside className={styles.personalNote}>
                  <strong>My guide note</strong>
                  <StudyMarkdown text={guideNote.plainText} />
                </aside>
              ) : null}
            </section>
            {guide.content.sections.map((section, index) => (
              <CollectionSection
                key={section.blockKey}
                section={section}
                index={index}
                collectionId={collectionId}
                overlays={guide.overlays}
                onOpenSource={() => void openSource(section)}
              />
            ))}
          </article>
        </div>
      )}
    </InlineWorkspace>
  );
}

function CollectionSection({
  section,
  index,
  collectionId,
  overlays,
  onOpenSource,
}: {
  section: CollectionGuideSection;
  index: number;
  collectionId: string;
  overlays: CollectionStudyGuideOverlay[];
  onOpenSource: () => void;
}) {
  const tutor = useOptionalTutor();
  const note = findCollectionOverlay(overlays, section.blockKey, "topic_note", "active");
  const override = findCollectionOverlay(overlays, section.blockKey, "body_override", "active");
  const conflict = findCollectionOverlay(overlays, section.blockKey, "body_override", "conflicted");
  const titleOverride = findCollectionOverlay(overlays, section.blockKey, "title_override", "active");
  const title = titleOverride?.plainText.trim() || section.title;
  const menuItems: OverflowMenuItem[] = [
    ...(tutor ? [{
      id: "tutor",
      label: "Ask Tutor about this section",
      disabled: tutor.busy || tutor.pending !== null,
      onSelect: () => tutor.openContext({
        title,
        scope: { kind: "collection", id: collectionId },
        focus: null,
        mode: "explain",
      }),
    }] : []),
    ...(section.sourceDocumentId ? [{
      id: "source",
      label: "Open source",
      onSelect: onOpenSource,
    }] : []),
  ];
  const source = collectionSourceLabel(section);
  return (
    <section id={`collection-guide-section-${index}`} className={styles.section}>
      <header className={styles.sectionHeader}>
        <div className={styles.sectionHeading}>
          {source ? <p className={styles.sourceLabel}>{source}</p> : null}
          <h2><StudyMarkdown text={title} inline /></h2>
        </div>
        {menuItems.length > 0 ? (
          <OverflowMenu
            label={`Section actions for ${title}`}
            items={menuItems}
            className={styles.sectionMenu}
          />
        ) : null}
      </header>
      {override ? (
        <div className={styles.overrideText}><StudyMarkdown text={override.plainText} /></div>
      ) : (
        <StudyGuideStructuredContent content={section.content} />
      )}
      {note?.plainText ? (
        <aside className={styles.personalNote}><strong>My note</strong><StudyMarkdown text={note.plainText} /></aside>
      ) : null}
      {conflict ? (
        <InlineNotice kind="warning">
          <strong>Older section edit needs review.</strong>
          <p>The collection guide changed, so the older edit was preserved without silently applying it.</p>
          <div className={styles.overrideText}><StudyMarkdown text={conflict.plainText} /></div>
        </InlineNotice>
      ) : null}
    </section>
  );
}

function findCollectionOverlay(
  overlays: CollectionStudyGuideOverlay[],
  blockKey: string,
  kind: string,
  state: string,
): CollectionStudyGuideOverlay | null {
  return overlays.find((overlay) =>
    overlay.blockKey === blockKey
    && overlay.overlayKind === kind
    && overlay.state === state) ?? null;
}

function collectionSourceLabel(section: CollectionGuideSection): string {
  const document = section.sourceDocumentTitle?.trim() ?? "";
  const locator = section.sourceLocator ? formatSourceLocator(section.sourceLocator) : "";
  return [document, locator].filter(Boolean).join(" · ");
}

function formatSourceLocator(locator: { unitKind: string; unitNumbers: number[] }): string {
  const numbers = locator.unitNumbers;
  const kind = locator.unitKind.replaceAll("_", " ");
  if (numbers.length === 0) return kind;
  const first = numbers[0];
  const last = numbers[numbers.length - 1];
  const label = first === last ? `${first}` : `${first}–${last}`;
  return `${capitalize(kind)} ${label}`;
}

function capitalize(value: string): string {
  return value.length === 0 ? value : `${value[0].toUpperCase()}${value.slice(1)}`;
}

function CollectionGuideList({ title, values }: { title: string; values: string[] }) {
  if (values.length === 0) return null;
  return <section><h3>{title}</h3><ul>{values.map((value) => (
    <li key={value}><StudyMarkdown text={value} /></li>
  ))}</ul></section>;
}

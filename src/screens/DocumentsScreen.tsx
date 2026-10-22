import { type ReactNode, useCallback, useMemo, useRef, useState } from "react";
import type { ShowToast } from "../components/Toast";
import { DocumentJobsProvider, useDocumentJobs } from "../controllers/DocumentJobsProvider";
import { isActiveArtifactJob, useDocumentArtifactProgress } from "../controllers/useDocumentArtifactProgress";
import type { DocumentGenerationProgress, DocumentSummary } from "../ipc/documents";
import { generationInstructionsError } from "../ipc/generationInstructions";
import type { DocumentArtifactJob } from "../ipc/studyGuides";
import { Icon } from "../ui/icons/Icon";
import { useLayoutMode } from "../ui/layout/LayoutModeProvider";
import { Dialog } from "../ui/overlays/Dialog";
import { MetadataDialog, MetadataItem, MetadataList } from "../ui/overlays/MetadataDialog";
import { OverflowMenu, type OverflowMenuItem } from "../ui/overlays/OverflowMenu";
import { PageHeader, PageScaffold } from "../ui/patterns/WorkspacePatterns";
import { Button } from "../ui/primitives/Button";
import { Badge, type BadgeTone } from "../ui/primitives/Badge";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { EmptyState, LoadingState } from "../ui/primitives/EmptyState";
import { Textarea } from "../ui/primitives/FormControls";
import { TextField } from "../ui/primitives/TextField";
import { FormField } from "../ui/primitives/FormField";
import {
  DocumentRowProgress, GenerationDiagnostics, GenerationProgressPanel, generationPercent, statusLabel, processingPhaseLabel, formatDocumentType,
  sourceUnitHeading, formatSourceUnitMetric, formatBytes, formatProcessingDuration,
  formatApiTokens, formatDate,
} from "./documents/DocumentProgress";
import { combinedArtifactProgress, LearningArtifactProgressPanel } from "./documents/LearningArtifactProgress";
import { GenerationSummary } from "./documents/GenerationSummary";
import { CheckedLearningPlanPanel } from "./documents/CheckedLearningPlanPanel";
import { ImageSetWorkspace } from "./documents/ImageSetWorkspace";
import { canAnnotateDocument } from "./documents/annotator/canAnnotate";
import { DocumentAnnotator } from "./documents/annotator/DocumentAnnotator";
import { DocumentFiguresWorkspace } from "./documents/figures/DocumentFiguresWorkspace";
import { ImportImagesDialog } from "./documents/figures/ImportImagesDialog";
import type { ImportDocumentOptions } from "../ipc/documents";
import type { StagedExternalFile } from "../ipc/externalFiles";
import { ReadingRangeDialog, rangeLabel } from "./documents/ReadingRangeDialog";
import { SubjectStudyActions } from "./documents/SubjectStudyActions";
import { useGenerationModelMissing } from "./documents/useGenerationModelMissing";
import { useDocumentFiling } from "./documents/useDocumentFiling";
import { CollectionStudyGuideDialog } from "./CollectionStudyGuideDialog";
import { subjectColorIndexes, subjectInitials } from "./home/homeModel";
import { SubjectBadge } from "./home/SubjectBadge";
import { MoveToSubjectSheet } from "./library/workbench/WorkbenchSheets";

interface DocumentsScreenProps {
  refreshDecks: () => Promise<unknown>;
  /** Practises cards made from a document's marks. */
  onPracticeCards?: (cardIds: string[]) => Promise<void>;
  /** Studies the decks a subject's study materials made. */
  onStudySubject?: (subjectId: string) => void;
  /** Opens a subject's own Documents page from Everything's groups; `null` is No subject. */
  onOpenSubject?: (subjectId: string | null) => void;
  showToast: ShowToast;
  /** Opens AI setup; Make actions use it in place of their work while no model is set. */
  onSetUpAi?: () => void;
  /** Shows one subject's documents, or No subject's (`null`); every document, grouped by subject, when left out. */
  subjectId?: string | null;
}

type DocumentsScreenJobs = NonNullable<ReturnType<typeof useDocumentJobs>>;

interface DocumentsScreenContentProps extends DocumentsScreenProps {
  jobs: DocumentsScreenJobs;
}

export function DocumentsScreen(props: DocumentsScreenProps) {
  const jobs = useDocumentJobs();
  if (jobs) return <DocumentsScreenContent {...props} jobs={jobs} />;
  return (
    <DocumentJobsProvider refreshDecks={props.refreshDecks} showToast={props.showToast}>
      <ProvidedDocumentsScreen {...props} />
    </DocumentJobsProvider>
  );
}

function ProvidedDocumentsScreen(props: DocumentsScreenProps) {
  const jobs = useDocumentJobs();
  if (!jobs) return null;
  return <DocumentsScreenContent {...props} jobs={jobs} />;
}

function DocumentsScreenContent(input: DocumentsScreenContentProps) {
  const { jobs, ...props } = input;
  const {
    documents, generationProgress, learningPlans, clockNow, isLoading, isUploading, busyDocumentIds,
    cancelingDocumentIds, deletingDocumentIds, guideGeneratingDocumentIds,
    cancelingGuideDocumentIds, errorMessage, uploadDocument, retry, generateMore,
    generateStudyGuide, stopStudyGuide, stopGeneration, openSource, renameDocument, deleteDocument,
  } = jobs;
  const artifactProgress = useDocumentArtifactProgress(
    isUploading || documents.some((document) => document.lifecycleState === "processing")
      || busyDocumentIds.size > 0 || guideGeneratingDocumentIds.size > 0,
  );
  // Phones drop the row's Open button, so Open leads the row's menu there.
  const compact = useLayoutMode().layoutMode === "compact";
  const filing = useDocumentFiling();
  const modelMissing = useGenerationModelMissing();
  const setUpAi = modelMissing ? props.onSetUpAi : undefined;
  const [moreCards, setMoreCards] = useState<{ document: DocumentSummary; instructions: string } | null>(null);
  const [imageWorkspaceDocumentId, setImageWorkspaceDocumentId] = useState<
    string | null | undefined
  >(undefined);
  const [subjectGuide, setSubjectGuide] = useState<{ id: string; title: string } | null>(null);
  const [annotating, setAnnotating] = useState<{ id: string; title: string } | null>(null);
  // The guide a Remake replaces, by document. Its finished record stays the
  // latest one until the new run has planned its sections, so it is not shown.
  const [remadeGuideJobIds, setRemadeGuideJobIds] = useState<Record<string, string>>({});
  const closeAnnotator = useCallback(() => setAnnotating(null), []);
  const [viewingFigures, setViewingFigures] = useState<{ id: string; title: string } | null>(null);
  const closeFigures = useCallback(() => setViewingFigures(null), []);
  // A chosen PDF waits here while the learner says whether to use its images.
  const [importing, setImporting] = useState<{
    fileName: string;
    resolve: (options: ImportDocumentOptions | null) => void;
  } | null>(null);
  const chooseImportOptions = (selected: StagedExternalFile) => selected.detectedKind === "pdf"
    ? new Promise<ImportDocumentOptions | null>((resolve) => setImporting({ fileName: selected.displayName, resolve }))
    : Promise.resolve({});
  const finishImporting = (options: ImportDocumentOptions | null) => {
    importing?.resolve(options);
    setImporting(null);
  };
  const [detailsDocumentId, setDetailsDocumentId] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<DocumentSummary | null>(null);
  const [moving, setMoving] = useState<DocumentSummary | null>(null);
  const [ranging, setRanging] = useState<DocumentSummary | null>(null);
  // An upload into a subject files the document once the picker returns.
  const latestFiling = useRef(filing);
  latestFiling.current = filing;

  const library = filing.library;
  const subjects = useMemo(
    () => [...(library?.subjects ?? [])].sort((left, right) => left.sortOrder - right.sortOrder),
    [library],
  );
  const colors = useMemo(() => subjectColorIndexes(subjects), [subjects]);
  const placements = useMemo(
    () => new Map((library?.documents ?? []).map((placement) => [placement.documentId, placement.subjectId])),
    [library],
  );
  const subjectOf = (document: DocumentSummary) => placements.get(document.id) ?? null;
  const [query, setQuery] = useState("");
  const search = query.trim().toLocaleLowerCase();
  // One group per subject that holds documents, in the learner's order, then No subject.
  const filedGroups = useMemo(() => {
    const filed = (subjectId: string | null) => documents.filter((document) => (placements.get(document.id) ?? null) === subjectId);
    if (props.subjectId !== undefined) return [{ subjectId: props.subjectId, name: "", documents: filed(props.subjectId) }];
    return [
      ...subjects.map((subject) => ({ subjectId: subject.id as string | null, name: subject.name, documents: filed(subject.id) })),
      { subjectId: null, name: "No subject", documents: filed(null) },
    ].filter((group) => group.documents.length > 0);
  }, [documents, placements, props.subjectId, subjects]);
  const filedCount = filedGroups.reduce((sum, group) => sum + group.documents.length, 0);
  // Search narrows each group to the documents whose title or file name matches.
  const groups = useMemo(
    () => search === ""
      ? filedGroups
      : filedGroups
        .map((group) => ({ ...group, documents: group.documents.filter((document) => matchesSearch(document, search)) }))
        .filter((group) => group.documents.length > 0),
    [filedGroups, search],
  );
  const shownCount = groups.reduce((sum, group) => sum + group.documents.length, 0);
  const subjectId = typeof props.subjectId === "string" ? props.subjectId : null;
  // The subject's materials reread their status whenever its documents or ranges change.
  const materialsKey = subjectId === null || !library
    ? 0
    : `${library.documentFilingRevision}:${filedCount}`;

  const openDocument = (document: DocumentSummary) => document.sourceFormat === "image_bundle"
    ? setImageWorkspaceDocumentId(document.id) : void openSource(document);
  const upload = () => {
    const target = props.subjectId;
    if (typeof target !== "string") return void uploadDocument(undefined, undefined, chooseImportOptions);
    return void uploadDocument(undefined, (documentId) => latestFiling.current.moveDocument(documentId, target), chooseImportOptions);
  };

  if (subjectGuide) {
    return (
      <PageScaffold width="full" aria-label="Subject study guide">
        <CollectionStudyGuideDialog
          collectionId={subjectGuide.id}
          collectionTitle={subjectGuide.title}
          showToast={props.showToast}
          onClose={() => setSubjectGuide(null)}
        />
      </PageScaffold>
    );
  }

  if (annotating) {
    return (
      <DocumentAnnotator
        documentId={annotating.id}
        title={annotating.title}
        onClose={closeAnnotator}
        showToast={props.showToast}
        onPracticeCards={props.onPracticeCards}
        onCardsChanged={() => void props.refreshDecks()}
      />
    );
  }

  if (viewingFigures) {
    return (
      <DocumentFiguresWorkspace
        documentId={viewingFigures.id}
        title={viewingFigures.title}
        onClose={closeFigures}
        showToast={props.showToast}
      />
    );
  }

  if (imageWorkspaceDocumentId !== undefined) {
    return (
      <ImageSetWorkspace
        documentId={imageWorkspaceDocumentId}
        onClose={() => setImageWorkspaceDocumentId(undefined)}
      />
    );
  }

  const detailsDocument = detailsDocumentId ? documents.find((document) => document.id === detailsDocumentId) : undefined;

  /** The guide job to show progress for. A remake hides the finished job it replaces until the new one appears. */
  function shownGuideJob(document: DocumentSummary) {
    const latest = artifactProgress.jobs[document.id] ?? null;
    const remaking = guideGeneratingDocumentIds.has(document.id)
      && latest !== null && remadeGuideJobIds[document.id] === latest.id;
    return remaking ? null : latest;
  }

  /** What a document is making and how far it has got, for Generation details. */
  function progressPanel(document: DocumentSummary) {
    const guideJob = shownGuideJob(document);
    const guideGenerating = guideGeneratingDocumentIds.has(document.id) || isActiveArtifactJob(guideJob);
    const live = generationProgress[document.id] ?? null;
    const processing = document.lifecycleState === "processing";
    const authoring = ["generating", "curating", "committing"].includes(live?.phase ?? "");
    const guideUnsettled = guideJob !== null && guideJob.state !== "succeeded" && guideJob.state !== "canceled";
    if (authoring || guideGenerating || (!processing && guideUnsettled)) {
      return <LearningArtifactProgressPanel document={document} progress={live} guideJob={guideJob}
        guideStarting={guideGenerating} readFailed={artifactProgress.readFailed} />;
    }
    return processing ? <GenerationProgressPanel document={document} progress={live} /> : null;
  }

  function renderRow(document: DocumentSummary) {
    const latestGuideJob = artifactProgress.jobs[document.id] ?? null;
    const guideJob = shownGuideJob(document);
    const guideGenerating = guideGeneratingDocumentIds.has(document.id) || isActiveArtifactJob(guideJob);
    const cardBusy = busyDocumentIds.has(document.id);
    const deleting = deletingDocumentIds.has(document.id);
    const busy = cardBusy || deleting || guideGenerating;
    const instructions = jobs.generationInstructionsFor(document);
    const canceling = cancelingDocumentIds.has(document.id);
    const cancelingGuide = cancelingGuideDocumentIds.has(document.id);
    const live = generationProgress[document.id] ?? null;
    const learningPlan = learningPlans[document.id] ?? null;
    const processing = document.lifecycleState === "processing";
    const authoring = ["generating", "curating", "committing"].includes(live?.phase ?? "");
    // A finished guide needs no panel; a running or failed one does.
    const guideUnsettled = guideJob !== null && guideJob.state !== "succeeded" && guideJob.state !== "canceled";
    const showArtifacts = authoring || guideGenerating || (!processing && guideUnsettled);
    const filedIn = subjectOf(document);
    const ranges = filing.rangesOf(document.id);
    const leftOut = filedIn !== null && filing.isLeftOut(document.id);
    const range = filedIn === null ? null : leftOut ? "left out of study materials" : rangeLabel(document, ranges);
    // A loose document's one filing action sits on the row; elsewhere it is in the menu.
    const addTo = filedIn === null && subjects.length > 0 && !processing;
    // Work under way shows as one bar in the row; a failed guide or card run keeps its panel below.
    const artifactBar = showArtifacts ? combinedArtifactProgress(document, live, guideJob, guideGenerating) : null;
    const rowProgress = artifactBar
      ?? (processing && !showArtifacts ? { label: processingPhaseLabel(document, canceling, live), value: generationPercent(document, live) } : null);
    const items: OverflowMenuItem[] = [];
    if (canAnnotateDocument(document)) items.push({
      id: "annotate", label: "Annotate", disabled: deleting,
      onSelect: () => setAnnotating({ id: document.id, title: document.title }),
    });
    if (canAnnotateDocument(document)) items.push({
      id: "figures", label: "Figures", disabled: deleting,
      onSelect: () => setViewingFigures({ id: document.id, title: document.title }),
    });
    if (filedIn !== null && document.sourceUnitCount > 0 && !processing) items.push({
      id: "range", label: "Reading range…", disabled: filing.busy,
      onSelect: () => setRanging(document),
    });
    if (filedIn !== null && !processing) items.push({
      id: "left-out", label: leftOut ? "Use in study materials" : "Leave out of study materials", disabled: filing.busy,
      onSelect: () => {
        const others = (library?.leftOutDocumentIds ?? []).filter((id) => id !== document.id && (placements.get(id) ?? null) === filedIn);
        void filing.setLeftOut(filedIn, leftOut ? others : [...others, document.id]);
      },
    });
    if (subjects.length > 0 && !processing) items.push({
      id: "move", label: filedIn === null ? "Add to a subject…" : "Move to another subject…", disabled: filing.busy,
      onSelect: () => setMoving(document),
    });
    if (filedIn !== null && !processing) items.push({
      id: "unfile", label: "Remove from subject", disabled: filing.busy,
      onSelect: () => void filing.moveDocument(document.id, null),
    });
    if (document.generatedDeckId !== null && document.lifecycleState === "ready") items.push({
      id: "generate-more", label: cardBusy ? "Making cards…" : "Make more cards…", disabled: busy,
      onSelect: setUpAi ?? (() => setMoreCards({ document, instructions })),
    });
    if (document.generatedDeckId === null && !processing) items.push({
      id: "retry", label: cardBusy ? "Retrying…" : "Retry", disabled: busy || generationInstructionsError(instructions) !== null,
      onSelect: setUpAi ?? (() => void retry(document, instructions)),
    });
    if (!guideGenerating && !processing) items.push({
      id: "study-guide", label: guideJob === null ? "Make a study guide" : "Remake the study guide", disabled: busy,
      onSelect: setUpAi ?? (() => {
        if (guideJob) setRemadeGuideJobIds((ids) => ({ ...ids, [document.id]: guideJob.id }));
        void generateStudyGuide(document);
      }),
    });
    if (guideGenerating && !processing) items.push({
      id: "stop-guide", label: cancelingGuide ? "Stopping study guide…" : "Stop study guide", disabled: cancelingGuide,
      onSelect: () => void stopStudyGuide(document),
    });
    if (processing) items.push({
      id: "stop-generation", label: canceling ? "Stopping…" : "Stop", destructive: true, disabled: canceling,
      onSelect: () => void stopGeneration(document),
    });
    if (!processing) items.push({ id: "rename", label: "Rename…", disabled: deleting, onSelect: () => setRenaming(document) });
    if (rowProgress) items.unshift({ id: "details", label: "Generation details", onSelect: () => setDetailsDocumentId(document.id) });
    else items.push({ id: "details", label: "Details", onSelect: () => setDetailsDocumentId(document.id) });
    if (!processing) items.push({
      id: "delete", label: deleting ? "Deleting…" : "Delete", destructive: true, disabled: busy,
      onSelect: () => void deleteDocument(document),
    });
    const openDisabled = document.sourceFormat === "image_bundle" ? deleting : busy;
    return (
      <li
        key={document.id}
        data-testid="document-row"
        aria-labelledby={`document-${document.id}`}
        className="tw:grid tw:min-w-0 tw:gap-3 tw:py-3 tw:not-last:border-0 tw:not-last:border-b tw:not-last:border-solid tw:not-last:border-border"
      >
        <div className="tw:flex tw:min-w-0 tw:items-center tw:gap-4 tw:compact:gap-3">
          <DocumentThumb image={document.sourceFormat === "image_bundle"} />
          <div className={rowProgress ? "tw:grid tw:min-w-0 tw:flex-1 tw:gap-1.5" : "tw:grid tw:min-w-0 tw:flex-1"}>
            <h3 id={`document-${document.id}`} className="tw:m-0 tw:wrap-anywhere tw:text-body tw:font-semibold">{document.title}</h3>
            {rowProgress ? <DocumentRowProgress label={rowProgress.label} value={rowProgress.value} /> : (
              <span className="tw:truncate tw:text-small tw:text-muted-foreground" title={document.originalFilename}>
                {[documentMeta(document), range].filter(Boolean).join(" · ")}
                <span className="tw:hidden tw:compact:inline"> · {materialSummary(document, latestGuideJob)}</span>
              </span>
            )}
          </div>
          {rowProgress ? null : <>
            <span className="tw:w-44 tw:shrink-0 tw:text-small tw:text-muted-foreground tw:tabular-nums tw:compact:hidden tw:medium:w-auto">
              {materialSummary(document, latestGuideJob)}
            </span>
            {document.lifecycleState === "ready" ? null : (
              <Badge tone={statusTone(document)} className="tw:shrink-0 tw:compact:hidden">{statusLabel(document)}</Badge>
            )}
          </>}
          {rowProgress ? null : addTo ? (
            <Button size="small" className="tw:shrink-0 tw:compact:hidden" disabled={filing.busy} onClick={() => setMoving(document)}>
              Add to…
            </Button>
          ) : (
            <Button size="small" className="tw:shrink-0 tw:compact:hidden" disabled={openDisabled} onClick={() => openDocument(document)}>
              Open
            </Button>
          )}
          <OverflowMenu
            label={`More actions for ${document.title}`}
            items={compact || addTo || rowProgress ? [{ id: "open", label: "Open", onSelect: () => openDocument(document), disabled: openDisabled }, ...items] : items}
          />
        </div>
        {processing || guideGenerating ? (
          <span className="tw:sr-only" aria-live="polite">
            {processing ? processingPhaseLabel(document, canceling, live) : cancelingGuide ? "Stopping study guide" : "Making study guide"}
          </span>
        ) : null}
        {showArtifacts && !artifactBar ? (
          <LearningArtifactProgressPanel
            document={document}
            progress={live}
            guideJob={guideJob}
            guideStarting={guideGenerating}
            readFailed={artifactProgress.readFailed}
          />
        ) : null}
        {processing && learningPlan ? (
          <CheckedLearningPlanPanel document={document} plan={learningPlan} />
        ) : null}
        {document.safeErrorMessage ? <InlineNotice kind="warning">{document.safeErrorMessage}</InlineNotice> : null}
      </li>
    );
  }

  const listClass = "tw:m-0 tw:grid tw:min-w-0 tw:list-none tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:px-4 tw:py-1 tw:compact:px-3";

  return (
    <PageScaffold width="full" role="region" aria-label="Documents">
      <PageHeader
        title="Documents"
        compact
        actions={
          <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-end tw:gap-2">
            {subjectId !== null ? (
              <SubjectStudyActions
                key={subjectId}
                subjectId={subjectId}
                refreshKey={materialsKey}
                hasDocuments={filedCount > 0}
                documents={filedGroups[0]?.documents ?? []}
                library={library}
                onSetLeftOut={(documentIds) => filing.setLeftOut(subjectId, documentIds)}
                onMakeFirstCards={(document, instructions, deckId) => retry(document, instructions, deckId)}
                isDocumentWorking={(documentId) => busyDocumentIds.has(documentId)
                  || documents.some((document) => document.id === documentId && document.lifecycleState === "processing")}
                refreshDecks={props.refreshDecks}
                onStudy={props.onStudySubject}
                onOpenGuide={(id, title) => setSubjectGuide({ id, title })}
                onSetUpAi={setUpAi}
              />
            ) : null}
            <OverflowMenu
              label={isUploading ? "Choose a document…" : "Upload"}
              triggerPresentation="text"
              triggerVariant="primary"
              disabled={isUploading}
              items={[
                { id: "document", label: "Document…", onSelect: upload },
                { id: "images", label: "Images…", onSelect: () => setImageWorkspaceDocumentId(null) },
              ]}
            />
          </div>
        }
      />
      {filedCount > 0 ? (
        <div role="search" className="tw:mb-4 tw:flex tw:min-w-0">
          <label className="tw:relative tw:flex tw:w-full tw:max-w-80 tw:items-center tw:compact:max-w-none">
            <span className="tw:sr-only">Search documents</span>
            <Icon name="search" size={16} className="tw:pointer-events-none tw:absolute tw:left-3 tw:text-muted-foreground" />
            <TextField
              type="search"
              value={query}
              placeholder="Search documents"
              className="tw:w-full tw:pl-9"
              onChange={(event) => setQuery(event.currentTarget.value)}
            />
          </label>
        </div>
      ) : null}
      {errorMessage ? <InlineNotice kind="error" className="tw:mb-5">{errorMessage}</InlineNotice> : null}
      {filing.error ? <InlineNotice kind="error" className="tw:mb-5">{filing.error}</InlineNotice> : null}
      {isLoading ? <LoadingState label="Loading document library…" /> : documents.length === 0 ? (
        <EmptyState title="No documents yet" icon="document" />
      ) : filedCount === 0 ? (
        <EmptyState title={props.subjectId === null ? "Every document has a subject" : "No documents in this subject yet"} icon="document" />
      ) : shownCount === 0 ? (
        <EmptyState title={`No documents match “${query.trim()}”`} icon="search" />
      ) : props.subjectId !== undefined ? (
        <section aria-labelledby="documents-list-heading" className="tw:grid tw:min-w-0">
          <h2 id="documents-list-heading" className="tw:sr-only">Documents</h2>
          <ul className={listClass}>{groups[0]?.documents.map(renderRow)}</ul>
        </section>
      ) : (
        <div className="tw:grid tw:min-w-0 tw:gap-6">
          {groups.map((group) => {
            const headingId = `documents-group-${group.subjectId ?? "none"}`;
            return (
              <section key={headingId} aria-labelledby={headingId} data-testid="document-group" className="tw:grid tw:min-w-0 tw:gap-2.5">
                <div className="tw:flex tw:min-w-0 tw:items-center tw:gap-2.5">
                  <SubjectBadge
                    subject={{
                      subjectId: group.subjectId,
                      initials: subjectInitials(group.name),
                      colorIndex: group.subjectId === null ? null : (colors.get(group.subjectId) ?? null),
                    }}
                    size="small"
                    className="tw:size-6 tw:text-caption"
                  />
                  <h2 id={headingId} className="tw:m-0 tw:min-w-0 tw:truncate tw:text-body tw:font-semibold">
                    {group.name}
                    <span className="tw:ml-2 tw:text-small tw:font-normal tw:text-muted-foreground">
                      {group.documents.length.toLocaleString()} document{group.documents.length === 1 ? "" : "s"}
                    </span>
                  </h2>
                  {props.onOpenSubject ? (
                    <Button
                      variant="text"
                      size="small"
                      className="tw:ml-auto tw:shrink-0"
                      aria-label={`Open ${group.name}`}
                      onClick={() => props.onOpenSubject?.(group.subjectId)}
                    >
                      Open subject <Icon name="chevron-right" size={16} />
                    </Button>
                  ) : null}
                </div>
                <ul className={listClass}>{group.documents.map(renderRow)}</ul>
              </section>
            );
          })}
        </div>
      )}
      {importing ? (
        <ImportImagesDialog
          fileName={importing.fileName}
          onImport={(useImages) => finishImporting({ useImages })}
          onCancel={() => finishImporting(null)}
        />
      ) : null}
      {detailsDocument ? (
        <DocumentDetails
          document={detailsDocument}
          progress={generationProgress[detailsDocument.id] ?? null}
          progressPanel={progressPanel(detailsDocument)}
          now={clockNow}
          onClose={() => setDetailsDocumentId(null)}
        />
      ) : null}
      {renaming ? (
        <RenameDocumentDialog
          document={renaming}
          onClose={() => setRenaming(null)}
          onSave={(title) => renameDocument(renaming, title)}
        />
      ) : null}
      {moving ? (
        <MoveToSubjectSheet
          itemLabel={`“${moving.title}”`}
          subjects={subjects}
          currentSubjectId={subjectOf(moving) ?? undefined}
          onClose={() => setMoving(null)}
          onMove={(target) => filing.moveDocument(moving.id, target)}
        />
      ) : null}
      {ranging ? (
        <ReadingRangeDialog
          document={ranging}
          ranges={filing.rangesOf(ranging.id)}
          busy={filing.busy}
          error={filing.error}
          onClose={() => setRanging(null)}
          onSave={(ranges) => filing.setRanges(ranging.id, ranges)}
        />
      ) : null}
      {moreCards ? (
        <MoreCardsDialog
          title={moreCards.document.title}
          instructions={moreCards.instructions}
          onChange={(instructions) => setMoreCards({ ...moreCards, instructions })}
          onClose={() => setMoreCards(null)}
          onSubmit={() => {
            const { document, instructions } = moreCards;
            setMoreCards(null);
            void generateMore(document, instructions);
          }}
        />
      ) : null}
    </PageScaffold>
  );
}

function DocumentThumb({ image }: { image: boolean }) {
  return (
    <span aria-hidden="true" className="tw:grid tw:h-13 tw:w-10 tw:shrink-0 tw:place-items-center tw:rounded-md tw:border tw:border-solid tw:border-border-strong tw:bg-muted/40 tw:text-muted-foreground tw:compact:h-11 tw:compact:w-8.5">
      <Icon name={image ? "image" : "document"} size={18} />
    </span>
  );
}

function documentMeta(document: DocumentSummary): string {
  const parts = [document.sourceFormat === "image_bundle" ? "Images" : formatDocumentType(document.sourceFormat)];
  if (document.sourceUnitCount > 0) {
    parts.push(`${document.sourceUnitCount.toLocaleString()} ${sourceUnitHeading(document.sourceUnitKind).toLowerCase()}`);
  }
  parts.push(`added ${formatAddedDate(document.createdAt)}`);
  return parts.join(" · ");
}

function formatAddedDate(timestamp: number): string {
  const startOfDay = (value: Date) => new Date(value.getFullYear(), value.getMonth(), value.getDate()).getTime();
  const days = Math.round((startOfDay(new Date()) - startOfDay(new Date(timestamp))) / 86_400_000);
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  return new Date(timestamp).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function materialSummary(document: DocumentSummary, guideJob: DocumentArtifactJob | null): string {
  const cards = document.generatedCardCount;
  const parts = guideJob?.state === "succeeded" ? ["Study guide"] : [];
  if (cards > 0) parts.push(`${cards.toLocaleString()} card${cards === 1 ? "" : "s"}`);
  if (cards > 0 && !document.generatedDeckId) parts.push("deck deleted");
  return parts.length ? parts.join(" · ") : "Nothing made yet";
}

function statusTone(document: DocumentSummary): BadgeTone {
  if (document.lifecycleState === "ready") return "success";
  if (document.lifecycleState === "processing") return "accent";
  return "warning";
}

function DocumentDetails({ document, progress, progressPanel, now, onClose }: {
  document: DocumentSummary;
  progress: DocumentGenerationProgress | null;
  /** While the document is being processed or made into study material; the dialog is then Generation details. */
  progressPanel: ReactNode;
  now: number;
  onClose: () => void;
}) {
  return (
    <MetadataDialog open title={progressPanel ? "Generation details" : "Details"} description={document.title} onClose={onClose}>
      {progressPanel}
      <MetadataList>
        <MetadataItem label="File" value={document.originalFilename} />
        <MetadataItem label="Format and size" value={`${formatDocumentType(document.sourceFormat)} · ${formatBytes(document.byteSize)}`} />
        <MetadataItem label={sourceUnitHeading(document.sourceUnitKind)} value={formatSourceUnitMetric(document)} />
        <MetadataItem label="Processing time" value={formatProcessingDuration(document.processingDurationMs)} />
        <MetadataItem label="API tokens" value={formatApiTokens(document, progress)} />
        <MetadataItem label="Updated" value={formatDate(document.updatedAt)} />
        <MetadataItem label="Created" value={formatDate(document.createdAt)} />
        <MetadataItem label="SHA-256" value={document.sha256} />
      </MetadataList>
      {progress ? <GenerationDiagnostics progress={progress} now={now} /> : null}
      <GenerationSummary report={document.generationSummary} />
    </MetadataDialog>
  );
}

function RenameDocumentDialog({ document, onClose, onSave }: {
  document: DocumentSummary;
  onClose: () => void;
  onSave: (title: string) => Promise<string | null>;
}) {
  const [title, setTitle] = useState(document.title);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const trimmed = title.trim();
  const unchanged = trimmed === document.title;
  return (
    <Dialog title="Rename document" width="small" busy={saving} onClose={onClose}
      onSubmit={() => void (async () => {
        if (!trimmed || saving) return;
        if (unchanged) { onClose(); return; }
        setSaving(true);
        const failure = await onSave(trimmed);
        setSaving(false);
        if (failure) setError(failure);
        else onClose();
      })()}
      actions={<>
        <Button onClick={onClose} disabled={saving}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!trimmed} loading={saving}>Save</Button>
      </>}>
      <FormField label="Title" error={error}>
        <TextField value={title} maxLength={200} autoFocus
          onChange={(event) => { setTitle(event.currentTarget.value); setError(null); }} />
      </FormField>
    </Dialog>
  );
}

function MoreCardsDialog({ title, instructions, onChange, onClose, onSubmit }: {
  title: string;
  instructions: string;
  onChange: (instructions: string) => void;
  onClose: () => void;
  onSubmit: () => void;
}) {
  const error = generationInstructionsError(instructions);
  return (
    <Dialog title="Make more cards" description={title} width="small" onClose={onClose}
      onSubmit={() => { if (!error) onSubmit(); }}
      actions={<>
        <Button onClick={onClose}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={error !== null}>Make cards</Button>
      </>}>
      <FormField label="Instructions (optional)" error={error}>
        <Textarea value={instructions} rows={4} autoFocus
          placeholder="For each cranial nerve, make concise recall cards about how nurses test it."
          onChange={(event) => onChange(event.currentTarget.value)} />
      </FormField>
    </Dialog>
  );
}

function matchesSearch(document: DocumentSummary, search: string): boolean {
  return document.title.toLocaleLowerCase().includes(search)
    || document.originalFilename.toLocaleLowerCase().includes(search);
}

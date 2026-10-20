import { type ReactNode, useCallback, useRef, useState } from "react";
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
  GenerationDiagnostics, GenerationProgressPanel, statusLabel, processingPhaseLabel, formatDocumentType,
  sourceUnitHeading, formatSourceUnitMetric, formatBytes, formatProcessingDuration,
  formatApiTokens, formatDate,
} from "./documents/DocumentProgress";
import { LearningArtifactProgressPanel } from "./documents/LearningArtifactProgress";
import { GenerationSummary } from "./documents/GenerationSummary";
import { CheckedLearningPlanPanel } from "./documents/CheckedLearningPlanPanel";
import { ImageSetWorkspace } from "./documents/ImageSetWorkspace";
import { canAnnotateDocument } from "./documents/annotator/canAnnotate";
import { DocumentAnnotator } from "./documents/annotator/DocumentAnnotator";
import { type CollectionDialog, DocumentCollectionsWorkspace } from "./documents/DocumentCollectionsWorkspace";
import { useDocumentCollectionsController } from "./documents/useDocumentCollectionsController";
import { CollectionStudyGuideDialog } from "./CollectionStudyGuideDialog";
import { beginDocumentFolderDrag, isInteractiveDocumentDragOrigin } from "./documents/documentFolderDrag";

interface DocumentsScreenProps {
  refreshDecks: () => Promise<unknown>;
  /** Practises cards made from a document's marks. */
  onPracticeCards?: (cardIds: string[]) => Promise<void>;
  /** Studies a collection live, as the decks filed in its subject. */
  onStudyCollection?: (collectionId: string) => void;
  showToast: ShowToast;
  headerSupplement?: ReactNode;
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
  const collections = useDocumentCollectionsController();
  // An upload into a collection files the document once the picker returns.
  const latestCollections = useRef(collections);
  latestCollections.current = collections;
  const [collectionDialog, setCollectionDialog] = useState<CollectionDialog | null>(null);
  const [moreCards, setMoreCards] = useState<{ document: DocumentSummary; instructions: string } | null>(null);
  const [imageWorkspaceDocumentId, setImageWorkspaceDocumentId] = useState<
    string | null | undefined
  >(undefined);
  const [collectionGuide, setCollectionGuide] = useState<{ id: string; title: string } | null>(null);
  const [annotating, setAnnotating] = useState<{ id: string; title: string } | null>(null);
  const closeAnnotator = useCallback(() => setAnnotating(null), []);
  const [detailsDocumentId, setDetailsDocumentId] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<DocumentSummary | null>(null);
  const [containedDocumentIds, setContainedDocumentIds] = useState<string[]>([]);
  const handleContainedDocumentIdsChange = useCallback((next: string[]) => {
    setContainedDocumentIds((current) => (
      current.length === next.length && current.every((id, index) => id === next[index])
        ? current
        : next
    ));
  }, []);
  const contained = new Set(containedDocumentIds);
  const looseDocuments = documents.filter((document) => !contained.has(document.id));
  const hasCollections = (collections.organization?.collections.length ?? 0) > 0;

  const openDocument = (document: DocumentSummary) => document.sourceFormat === "image_bundle"
    ? setImageWorkspaceDocumentId(document.id) : void openSource(document);
  const uploadInto = (collectionId: string) => void uploadDocument(undefined,
    (documentId) => latestCollections.current.addDocument(collectionId, documentId));

  if (collectionGuide) {
    return (
      <PageScaffold width="full" aria-label="Documents collection study guide">
        <CollectionStudyGuideDialog
          collectionId={collectionGuide.id}
          collectionTitle={collectionGuide.title}
          showToast={props.showToast}
          onClose={() => setCollectionGuide(null)}
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

  if (imageWorkspaceDocumentId !== undefined) {
    return (
      <ImageSetWorkspace
        documentId={imageWorkspaceDocumentId}
        onClose={() => setImageWorkspaceDocumentId(undefined)}
      />
    );
  }

  const detailsDocument = detailsDocumentId ? documents.find((document) => document.id === detailsDocumentId) : undefined;

  return (
    <PageScaffold width="full" role="region" aria-label="Documents">
      <PageHeader
        title="Documents"
        compact
        actions={
          <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2">
            <Button
              disabled={collections.organization === null || collections.busy}
              onClick={() => setCollectionDialog({ kind: "create", name: "" })}
            >
              New collection
            </Button>
            <OverflowMenu
              label={isUploading ? "Choose a document…" : "Upload"}
              triggerPresentation="text"
              triggerVariant="primary"
              disabled={isUploading}
              items={[
                { id: "document", label: "Document…", onSelect: () => void uploadDocument() },
                { id: "images", label: "Images…", onSelect: () => setImageWorkspaceDocumentId(null) },
              ]}
            />
          </div>
        }
      />
      {props.headerSupplement}
      {errorMessage ? <InlineNotice kind="error" className="tw:mb-5">{errorMessage}</InlineNotice> : null}
      <DocumentCollectionsWorkspace
        controller={collections}
        dialog={collectionDialog}
        onDialogChange={setCollectionDialog}
        documents={documents}
        refreshDecks={props.refreshDecks}
        onStudyCollection={props.onStudyCollection}
        onOpenGuide={(id, title) => setCollectionGuide({ id, title })}
        onOpenDocument={openDocument}
        onAnnotateDocument={(document) => setAnnotating({ id: document.id, title: document.title })}
        onRenameDocument={setRenaming}
        onUploadInto={uploadInto}
        onContainedDocumentIdsChange={handleContainedDocumentIdsChange}
      />
      {isLoading ? <LoadingState label="Loading document library…" /> : documents.length === 0 ? (
        <EmptyState title="No documents yet" icon="document" />
      ) : looseDocuments.length === 0 ? null : (
        <section aria-labelledby="loose-documents-heading" className="tw:grid tw:min-w-0 tw:gap-3">
          {hasCollections ? (
            <h2 id="loose-documents-heading" className="tw:m-0 tw:text-body tw:font-semibold">Documents</h2>
          ) : <h2 id="loose-documents-heading" className="sr-only">Documents</h2>}
          <ul className="tw:m-0 tw:grid tw:min-w-0 tw:list-none tw:gap-2 tw:p-0">
            {looseDocuments.map((document) => {
              const guideJob = artifactProgress.jobs[document.id] ?? null;
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
              const items: OverflowMenuItem[] = [];
              if (canAnnotateDocument(document)) items.push({
                id: "annotate", label: "Annotate", disabled: deleting,
                onSelect: () => setAnnotating({ id: document.id, title: document.title }),
              });
              if (hasCollections && !processing) items.push({
                id: "move", label: "Move to a collection…", disabled: collections.busy,
                onSelect: () => setCollectionDialog({ kind: "move-document", documentId: document.id, targetCollectionId: "" }),
              });
              if (document.generatedDeckId !== null && document.lifecycleState === "ready") items.push({
                id: "generate-more", label: cardBusy ? "Making cards…" : "Make more cards…", disabled: busy,
                onSelect: () => setMoreCards({ document, instructions }),
              });
              if (document.generatedDeckId === null && !processing) items.push({
                id: "retry", label: cardBusy ? "Retrying…" : "Retry", disabled: busy || generationInstructionsError(instructions) !== null,
                onSelect: () => void retry(document, instructions),
              });
              if (!guideGenerating && !processing) items.push({
                id: "study-guide", label: guideJob === null ? "Make a study guide" : "Remake the study guide", disabled: busy,
                onSelect: () => void generateStudyGuide(document),
              });
              if (guideGenerating && !processing) items.push({
                id: "stop-guide", label: cancelingGuide ? "Stopping study guide…" : "Stop study guide", disabled: cancelingGuide,
                onSelect: () => void stopStudyGuide(document),
              });
              if (processing) items.push({
                id: "stop-generation", label: canceling ? "Stopping…" : "Stop processing", disabled: canceling,
                onSelect: () => void stopGeneration(document),
              });
              items.push({ id: "rename", label: "Rename…", disabled: deleting, onSelect: () => setRenaming(document) });
              items.push({ id: "details", label: "Details", onSelect: () => setDetailsDocumentId(document.id) });
              if (!processing) items.push({
                id: "delete", label: deleting ? "Deleting…" : "Delete", destructive: true, disabled: busy,
                onSelect: () => void deleteDocument(document),
              });
              return (
                <li
                  key={document.id}
                  data-testid="document-row"
                  aria-labelledby={`document-${document.id}`}
                  className="tw:grid tw:min-w-0 tw:gap-3 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:py-3 tw:pr-3 tw:pl-4 tw:compact:pl-3"
                >
                  <div
                    className="tw:flex tw:min-w-0 tw:cursor-grab tw:items-center tw:gap-4 tw:active:cursor-grabbing tw:compact:gap-3"
                    data-document-drag-source={document.id}
                    draggable={!processing}
                    onDragStart={(event) => {
                      if (isInteractiveDocumentDragOrigin(event.target)) {
                        event.preventDefault();
                        return;
                      }
                      beginDocumentFolderDrag(event.dataTransfer, document.id);
                    }}
                  >
                    <DocumentThumb image={document.sourceFormat === "image_bundle"} />
                    <div className="tw:grid tw:min-w-0 tw:flex-1">
                      <h3 id={`document-${document.id}`} className="tw:m-0 tw:wrap-anywhere tw:text-body tw:font-semibold">{document.title}</h3>
                      <span className="tw:truncate tw:text-small tw:text-muted-foreground" title={document.originalFilename}>
                        {documentMeta(document)}
                        <span className="tw:hidden tw:compact:inline"> · {materialSummary(document, guideJob)}</span>
                      </span>
                    </div>
                    <span className="tw:shrink-0 tw:text-small tw:text-muted-foreground tw:tabular-nums tw:compact:hidden">
                      {materialSummary(document, guideJob)}
                    </span>
                    <Badge tone={statusTone(document)} className="tw:shrink-0 tw:compact:hidden">{statusLabel(document)}</Badge>
                    <Button size="small" className="tw:shrink-0 tw:compact:hidden"
                      disabled={document.sourceFormat === "image_bundle" ? deleting : busy}
                      onClick={() => openDocument(document)}>
                      Open
                    </Button>
                    <OverflowMenu label={`More actions for ${document.title}`} items={compact ? [{
                      id: "open", label: "Open", onSelect: () => openDocument(document),
                      disabled: document.sourceFormat === "image_bundle" ? deleting : busy,
                    }, ...items] : items} />
                  </div>
                  {processing || guideGenerating ? (
                    <span className="tw:sr-only" aria-live="polite">
                      {processing ? processingPhaseLabel(document, canceling, live) : cancelingGuide ? "Stopping study guide" : "Making study guide"}
                    </span>
                  ) : null}
                  {showArtifacts ? (
                    <LearningArtifactProgressPanel
                      document={document}
                      progress={live}
                      guideJob={guideJob}
                      guideStarting={guideGenerating}
                      readFailed={artifactProgress.readFailed}
                    />
                  ) : processing ? (
                    <GenerationProgressPanel document={document} progress={live} />
                  ) : null}
                  {processing && learningPlan ? (
                    <CheckedLearningPlanPanel document={document} plan={learningPlan} />
                  ) : null}
                  {document.safeErrorMessage ? <InlineNotice kind="warning">{document.safeErrorMessage}</InlineNotice> : null}
                </li>
              );
            })}
          </ul>
        </section>
      )}
      {detailsDocument ? (
        <DocumentDetails
          document={detailsDocument}
          progress={generationProgress[detailsDocument.id] ?? null}
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
  const parts = cards > 0 ? [`${cards.toLocaleString()} card${cards === 1 ? "" : "s"}`] : [];
  if (cards > 0 && !document.generatedDeckId) parts.push("deck deleted");
  if (guideJob?.state === "succeeded") parts.push("guide");
  return parts.length ? parts.join(" · ") : "Nothing yet";
}

function statusTone(document: DocumentSummary): BadgeTone {
  if (document.lifecycleState === "ready") return "success";
  if (document.lifecycleState === "processing") return "accent";
  return "warning";
}

function DocumentDetails({ document, progress, now, onClose }: {
  document: DocumentSummary;
  progress: DocumentGenerationProgress | null;
  now: number;
  onClose: () => void;
}) {
  return (
    <MetadataDialog open title="Details" description={document.title} onClose={onClose}>
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

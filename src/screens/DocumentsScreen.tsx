import { type ReactNode, useCallback, useState } from "react";
import type { ShowToast } from "../components/Toast";
import { DocumentJobsProvider, useDocumentJobs } from "../controllers/DocumentJobsProvider";
import { isActiveArtifactJob, useDocumentArtifactProgress } from "../controllers/useDocumentArtifactProgress";
import { generationInstructionsError } from "../ipc/generationInstructions";
import { MetadataDialog, MetadataItem, MetadataList } from "../ui/overlays/MetadataDialog";
import { ResponsiveActionGroup, type ResponsiveAction } from "../ui/patterns/ResponsiveActionGroup";
import { PageHeader, PageScaffold } from "../ui/patterns/WorkspacePatterns";
import { Button } from "../ui/primitives/Button";
import { Surface } from "../ui/primitives/Surface";
import { Badge } from "../ui/primitives/Badge";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { EmptyState, LoadingState } from "../ui/primitives/EmptyState";
import {
  GenerationProgressPanel, statusLabel, processingPhaseLabel, formatDocumentType,
  sourceUnitHeading, formatSourceUnitMetric, formatBytes, formatProcessingDuration,
  formatApiTokens, formatDate,
} from "./documents/DocumentProgress";
import { LearningArtifactProgressPanel } from "./documents/LearningArtifactProgress";
import { GenerationSummary } from "./documents/GenerationSummary";
import { GenerationInstructionsField } from "./documents/GenerationInstructionsField";
import { CheckedLearningPlanPanel } from "./documents/CheckedLearningPlanPanel";
import { ImageSetWorkspace } from "./documents/ImageSetWorkspace";
import { DocumentCollectionsWorkspace } from "./documents/DocumentCollectionsWorkspace";
import { CollectionStudyGuideDialog } from "./CollectionStudyGuideDialog";
import { beginDocumentFolderDrag, isInteractiveDocumentDragOrigin } from "./documents/documentFolderDrag";
import { documentStyles as styles } from "./documents/presentation";

interface DocumentsScreenProps {
  refreshDecks: () => Promise<unknown>;
  onOpenDeck?: (deckId: string) => void;
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
    generateStudyGuide, stopStudyGuide, stopGeneration, openSource, deleteDocument,
  } = jobs;
  const artifactProgress = useDocumentArtifactProgress(
    isUploading || documents.some((document) => document.lifecycleState === "processing")
      || busyDocumentIds.size > 0 || guideGeneratingDocumentIds.size > 0,
  );
  const [uploadInstructions, setUploadInstructions] = useState("");
  const [instructionDrafts, setInstructionDrafts] = useState<Record<string, string>>({});
  const [imageWorkspaceDocumentId, setImageWorkspaceDocumentId] = useState<
    string | null | undefined
  >(undefined);
  const [collectionGuide, setCollectionGuide] = useState<{ id: string; title: string } | null>(null);
  const [generationInfoDocumentId, setGenerationInfoDocumentId] = useState<string | null>(null);
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

  if (imageWorkspaceDocumentId !== undefined) {
    return (
      <ImageSetWorkspace
        documentId={imageWorkspaceDocumentId}
        onClose={() => setImageWorkspaceDocumentId(undefined)}
      />
    );
  }

  return (
    <PageScaffold role="region" aria-label="Documents">
      <PageHeader
        title="Documents"
        icon="document"
        description="Turn your course materials into connected flashcards and study guides."
        actions={
          <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2">
            <Button
              variant="secondary"
              disabled={isUploading}
              onClick={() => setImageWorkspaceDocumentId(null)}
            >
              Upload images
            </Button>
            <Button variant="primary"
              disabled={isUploading || generationInstructionsError(uploadInstructions) !== null}
              onClick={() => void uploadDocument(uploadInstructions)}>
              {isUploading ? "Choose a document…" : "Upload document"}
            </Button>
          </div>
        }
      />
      {props.headerSupplement}
      <GenerationInstructionsField compact label="Instructions for new document flashcards"
        value={uploadInstructions} onChange={setUploadInstructions} disabled={isUploading} />
      <DocumentCollectionsWorkspace
        documents={documents}
        refreshDecks={props.refreshDecks}
        onOpenDeck={props.onOpenDeck}
        onOpenGuide={(id, title) => setCollectionGuide({ id, title })}
        onContainedDocumentIdsChange={handleContainedDocumentIdsChange}
      />
      <div className={styles.toolbar}>
        <strong>Loose documents</strong>
        <span className={styles.count}>{looseDocuments.length} document{looseDocuments.length === 1 ? "" : "s"}</span>
      </div>
      {errorMessage ? <InlineNotice kind="error" className="tw:mb-5">{errorMessage}</InlineNotice> : null}
      {isLoading ? <LoadingState label="Loading document library…" /> : documents.length === 0 ? (
        <EmptyState title="No documents yet" description="Upload a course handout, chapter, presentation, or study packet to create learning materials." />
      ) : looseDocuments.length === 0 ? (
        <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
          Every document is currently organized into a collection.
        </p>
      ) : (
        <div className={styles.list}>
          {looseDocuments.map((document) => {
            const guideJob = artifactProgress.jobs[document.id] ?? null;
            const guideGenerating = guideGeneratingDocumentIds.has(document.id) || isActiveArtifactJob(guideJob);
            const cardBusy = busyDocumentIds.has(document.id);
            const deleting = deletingDocumentIds.has(document.id);
            const busy = cardBusy || deleting || guideGenerating;
            const instructions = instructionDrafts[document.id] ?? jobs.generationInstructionsFor(document);
            const invalidInstructions = generationInstructionsError(instructions) !== null;
            const canceling = cancelingDocumentIds.has(document.id);
            const cancelingGuide = cancelingGuideDocumentIds.has(document.id);
            const live = generationProgress[document.id] ?? null;
            const learningPlan = learningPlans[document.id] ?? null;
            const authoring = ["generating", "curating", "committing"].includes(live?.phase ?? "");
            const showArtifacts = authoring || guideGenerating
              || (document.lifecycleState !== "processing" && guideJob !== null);
            const canRetry = document.generatedDeckId === null && document.lifecycleState !== "processing";
            const canGenerateMore = document.generatedDeckId !== null && document.lifecycleState === "ready";
            const actions: ResponsiveAction[] = [{
              id: "view",
              label: document.sourceFormat === "image_bundle" ? "View image collection" : "View",
              placement: "always-visible",
              presentation: "icon",
              icon: "view",
              onSelect: () => document.sourceFormat === "image_bundle"
                ? setImageWorkspaceDocumentId(document.id) : void openSource(document),
              disabled: document.sourceFormat === "image_bundle" ? deleting : busy,
            }];
            if (document.lifecycleState !== "processing") actions.push({
              id: "delete",
              label: "Delete document",
              description: "Permanently delete this managed source and document-owned derivatives.",
              placement: "always-visible",
              presentation: "icon",
              icon: "delete",
              destructive: true,
              onSelect: () => void deleteDocument(document),
              disabled: busy,
              loading: deleting,
            });
            actions.push({
              id: "generation-info",
              label: "Generation info",
              placement: "overflow-only",
              onSelect: () => setGenerationInfoDocumentId(document.id),
            });
            if (document.lifecycleState === "processing") actions.push({
              id: "stop-generation",
              label: canceling ? "Stopping…" : "Stop generation",
              placement: "overflow-only",
              destructive: true,
              onSelect: () => void stopGeneration(document),
              disabled: canceling,
            });
            if (!guideGenerating && document.lifecycleState !== "processing") actions.push({
              id: "study-guide",
              label: guideJob === null ? "Generate study guide" : "Regenerate study guide",
              placement: "overflow-only",
              onSelect: () => void generateStudyGuide(document),
              disabled: busy,
            });
            if (guideGenerating && document.lifecycleState !== "processing") actions.push({
              id: "stop-guide",
              label: cancelingGuide ? "Stopping study guide…" : "Stop study guide generation",
              placement: "overflow-only",
              destructive: true,
              onSelect: () => void stopStudyGuide(document),
              disabled: cancelingGuide,
            });
            if (canRetry) actions.push({
              id: "retry",
              label: cardBusy ? "Retrying…" : "Retry generation",
              placement: "overflow-only",
              onSelect: () => void retry(document, instructions),
              disabled: busy || invalidInstructions,
            });
            if (canGenerateMore) actions.push({
              id: "generate-more",
              label: cardBusy ? "Generating…" : "Generate more cards",
              placement: "overflow-only",
              onSelect: () => void generateMore(document, instructions),
              disabled: busy || invalidInstructions,
            });
            return (
              <Surface as="article" key={document.id} className={styles.card}>
                <div
                  className={`${styles.cardMain} tw:cursor-grab tw:active:cursor-grabbing`}
                  data-document-drag-source={document.id}
                  draggable={document.lifecycleState !== "processing"}
                  onDragStart={(event) => {
                    if (isInteractiveDocumentDragOrigin(event.target)) {
                      event.preventDefault();
                      return;
                    }
                    beginDocumentFolderDrag(event.dataTransfer, document.id);
                  }}
                >
                  <div className={styles.titleRow}>
                    <h2>{document.title}</h2>
                    <Badge tone={document.lifecycleState === "ready" ? "success" : document.lifecycleState === "processing" ? "accent" : "warning"}>
                      {statusLabel(document)}
                    </Badge>
                    {document.lifecycleState === "processing" ? (
                      <span className={styles.processingPhase} aria-live="polite">{processingPhaseLabel(document, canceling, live)}</span>
                    ) : guideGenerating ? (
                      <span className={styles.processingPhase} aria-live="polite">{cancelingGuide ? "Stopping study guide" : "Generating study guide"}</span>
                    ) : null}
                  </div>
                  <div className={styles.sourceSummary}>
                    <span className={styles.filename}>{document.originalFilename}</span>
                    <span className={styles.cardCount}>{document.generatedCardCount} card{document.generatedCardCount === 1 ? "" : "s"}</span>
                  </div>
                  <GenerationInstructionsField compact value={instructions}
                    disabled={busy || document.lifecycleState === "processing"}
                    onChange={(value) => setInstructionDrafts((current) => ({ ...current, [document.id]: value }))} />
                  {showArtifacts ? (
                    <LearningArtifactProgressPanel
                      document={document}
                      progress={live}
                      guideJob={guideJob}
                      guideStarting={guideGenerating}
                      readFailed={artifactProgress.readFailed}
                      now={clockNow}
                    />
                  ) : document.lifecycleState === "processing" ? (
                    <GenerationProgressPanel document={document} progress={live} now={clockNow} />
                  ) : null}
                  {document.lifecycleState === "processing" && learningPlan ? (
                    <CheckedLearningPlanPanel document={document} plan={learningPlan} />
                  ) : null}
                  {document.safeErrorMessage ? <InlineNotice kind="warning" className="tw:mt-4">{document.safeErrorMessage}</InlineNotice> : null}
                  {!document.generatedDeckId && document.generatedCardCount > 0 && document.lifecycleState === "ready" ? (
                    <p className={styles.deckNote}>The previous generated deck was deleted. Retry generation to create a new deck from the retained document.</p>
                  ) : null}
                </div>
                <ResponsiveActionGroup
                  label={`Actions for ${document.title}`}
                  overflowLabel={`More actions for ${document.title}`}
                  actions={actions}
                  className={styles.actions}
                />
                {generationInfoDocumentId === document.id ? (
                  <MetadataDialog
                    open
                    title={`${document.title} generation info`}
                    description="Source, processing, and latest generation details for this document."
                    onClose={() => setGenerationInfoDocumentId(null)}
                  >
                    <MetadataList>
                      <MetadataItem label="Format and size" value={`${formatDocumentType(document.sourceFormat)} · ${formatBytes(document.byteSize)}`} />
                      <MetadataItem label={sourceUnitHeading(document.sourceUnitKind)} value={formatSourceUnitMetric(document)} />
                      <MetadataItem label="Processing time" value={formatProcessingDuration(document.processingDurationMs)} />
                      <MetadataItem label="API tokens" value={formatApiTokens(document, live)} />
                      <MetadataItem label="Updated" value={formatDate(document.updatedAt)} />
                      <MetadataItem label="Created" value={formatDate(document.createdAt)} />
                      <MetadataItem label="SHA-256" value={document.sha256} />
                    </MetadataList>
                    <GenerationSummary report={document.generationSummary} />
                  </MetadataDialog>
                ) : null}
              </Surface>
            );
          })}
        </div>
      )}
    </PageScaffold>
  );
}

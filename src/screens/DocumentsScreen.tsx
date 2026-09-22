import { type ReactNode, useState } from "react";
import type { ShowToast } from "../components/Toast";
import { DocumentJobsProvider, useDocumentJobs } from "../controllers/DocumentJobsProvider";
import { InfoButton, MetadataItem, MetadataList } from "../ui/overlays/MetadataDialog";
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
import { GenerationSummary } from "./documents/GenerationSummary";
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

export function DocumentsScreen(props: DocumentsScreenProps) {
  const jobs = useDocumentJobs();
  if (!jobs) return <DocumentJobsProvider refreshDecks={props.refreshDecks} showToast={props.showToast}>
    <DocumentsScreen {...props} />
  </DocumentJobsProvider>;
  const {
    documents, generationProgress, clockNow, isLoading, isUploading, busyDocumentId,
    cancelingDocumentId, deletingDocumentId, guideGeneratingDocumentId, cancelingGuideDocumentId,
    errorMessage, hasProcessingDocument, uploadDocument, retry, generateMore,
    generateStudyGuide, stopStudyGuide, stopGeneration, openSource, deleteDocument,
  } = jobs;
  const [imageWorkspaceDocumentId, setImageWorkspaceDocumentId] = useState<
    string | null | undefined
  >(undefined);
  const [collectionGuide, setCollectionGuide] = useState<{ id: string; title: string } | null>(null);
  const [containedDocumentIds, setContainedDocumentIds] = useState<string[]>([]);
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
              disabled={isUploading || hasProcessingDocument || busyDocumentId !== null}
              onClick={() => setImageWorkspaceDocumentId(null)}
            >
              Upload images
            </Button>
            <Button variant="primary"
              disabled={isUploading || busyDocumentId !== null || deletingDocumentId !== null || guideGeneratingDocumentId !== null}
              onClick={() => void uploadDocument()}>
              {isUploading ? "Processing document…" : "Upload document"}
            </Button>
          </div>
        }
      />
      {props.headerSupplement}
      <DocumentCollectionsWorkspace
        documents={documents}
        refreshDecks={props.refreshDecks}
        onOpenDeck={props.onOpenDeck}
        onOpenGuide={(id, title) => setCollectionGuide({ id, title })}
        onContainedDocumentIdsChange={setContainedDocumentIds}
      />
      <div className={styles.toolbar}>
        <strong>Loose documents</strong>
        <span className={styles.count}>{looseDocuments.length} document{looseDocuments.length === 1 ? "" : "s"}</span>
      </div>
      {errorMessage ? <InlineNotice kind="error" className="tw:mb-5">{errorMessage}</InlineNotice> : null}
      {isLoading ? <LoadingState label="Loading document library…" /> : documents.length === 0 ? (
        <EmptyState title="No documents yet" description="Upload a course handout, chapter, presentation, or study packet to create learning materials." />
      ) : looseDocuments.length === 0 ? (
        <p className="tw:m-0 tw:text-sm tw:text-muted-foreground">
          Every document is currently organized into a collection.
        </p>
      ) : (
        <div className={styles.list}>
          {looseDocuments.map((document) => {
            const guideGenerating = guideGeneratingDocumentId === document.id;
            const busy = busyDocumentId === document.id || deletingDocumentId === document.id || guideGenerating;
            const canceling = cancelingDocumentId === document.id;
            const cancelingGuide = cancelingGuideDocumentId === document.id;
            const live = generationProgress[document.id] ?? null;
            const canRetry = document.generatedDeckId === null && document.lifecycleState !== "processing";
            const canGenerateMore = document.generatedDeckId !== null && document.lifecycleState === "ready";
            const canGenerateGuide = document.lifecycleState !== "processing";
            const actions: ResponsiveAction[] = [];
            actions.push({
              id: "view", label: document.sourceFormat === "image_bundle" ? "View collection" : "View", placement: "always-visible",
              onSelect: () => document.sourceFormat === "image_bundle"
                ? setImageWorkspaceDocumentId(document.id) : void openSource(document),
              disabled: document.sourceFormat === "image_bundle" ? deletingDocumentId === document.id : busy,
            });
            if (document.lifecycleState === "processing") actions.push({
              id: "stop-generation", label: canceling ? "Stopping…" : "Stop generation",
              placement: "always-visible", onSelect: () => void stopGeneration(document), disabled: canceling,
            });
            if (canGenerateGuide) actions.push({
              id: "generate-guide",
              label: guideGenerating ? (cancelingGuide ? "Stopping study guide…" : "Generating study guide…") : "Generate study guide",
              description: guideGenerating ? "Stop study guide generation. Saved topics remain available." : undefined,
              icon: guideGenerating ? "stop" : undefined,
              destructive: guideGenerating, placement: "always-visible",
              variant: guideGenerating ? "secondary" : "primary",
              onSelect: () => void (guideGenerating ? stopStudyGuide(document) : generateStudyGuide(document)),
              disabled: guideGenerating ? cancelingGuide : busy || isUploading,
            });
            if (canRetry) actions.push({
              id: "retry", label: busyDocumentId === document.id ? "Retrying…" : "Retry generation",
              placement: "always-visible", variant: "primary",
              onSelect: () => void retry(document), disabled: busy || isUploading,
            });
            if (canGenerateMore) actions.push({
              id: "generate-more", label: busyDocumentId === document.id ? "Generating…" : "Generate more cards",
              placement: "wide-and-medium", variant: "primary",
              onSelect: () => void generateMore(document), disabled: busy || isUploading,
            });
            if (document.lifecycleState !== "processing") actions.push({
              id: "delete", label: "Delete document",
              description: "Permanently delete this managed source and document-owned derivatives.",
              placement: "wide-and-medium", presentation: "icon", icon: "delete", destructive: true,
              onSelect: () => void deleteDocument(document), disabled: busy || isUploading,
              loading: deletingDocumentId === document.id,
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
                    {document.lifecycleState !== "processing" ? (
                      <InfoButton label={`View metadata for ${document.title}`} title={`${document.title} metadata`}
                        description="Source and processing details for this document." className={styles.metadataButton}>
                        <MetadataList>
                          <MetadataItem label="Format and size" value={`${formatDocumentType(document.sourceFormat)} · ${formatBytes(document.byteSize)}`} />
                          <MetadataItem label={sourceUnitHeading(document.sourceUnitKind)} value={formatSourceUnitMetric(document)} />
                          <MetadataItem label="Processing time" value={formatProcessingDuration(document.processingDurationMs)} />
                          <MetadataItem label="API tokens" value={formatApiTokens(document, null)} />
                          <MetadataItem label="Updated" value={formatDate(document.updatedAt)} />
                          <MetadataItem label="Created" value={formatDate(document.createdAt)} />
                          <MetadataItem label="SHA-256" value={document.sha256} />
                        </MetadataList>
                        <GenerationSummary report={document.generationSummary} />
                      </InfoButton>
                    ) : null}
                  </div>
                  <div className={styles.sourceSummary}>
                    <span className={styles.filename}>{document.originalFilename}</span>
                    <span className={styles.cardCount}>{document.generatedCardCount} card{document.generatedCardCount === 1 ? "" : "s"}</span>
                  </div>
                  {document.lifecycleState === "processing" ? <GenerationProgressPanel document={document} progress={live} now={clockNow} /> : null}
                  {document.safeErrorMessage ? <InlineNotice kind="warning" className="tw:mt-4">{document.safeErrorMessage}</InlineNotice> : null}
                  {!document.generatedDeckId && document.generatedCardCount > 0 && document.lifecycleState === "ready" ? (
                    <p className={styles.deckNote}>The previous generated deck was deleted. Retry generation to create a new deck from the retained document.</p>
                  ) : null}
                </div>
                <ResponsiveActionGroup label={`Actions for ${document.title}`} actions={actions} className={styles.actions} />
              </Surface>
            );
          })}
        </div>
      )}
    </PageScaffold>
  );
}

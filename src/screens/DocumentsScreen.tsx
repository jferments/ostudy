import { type ReactNode } from "react";
import type { ShowToast } from "../components/Toast";
import { DocumentJobsProvider, useDocumentJobs } from "../controllers/DocumentJobsProvider";
import { InfoButton, MetadataItem, MetadataList } from "../ui/overlays/MetadataDialog";
import { ResponsiveActionGroup, type ResponsiveAction } from "../ui/patterns/ResponsiveActionGroup";
import { PageHeader } from "../ui/patterns/WorkspacePatterns";
import { ownedClassNames, sharedClassMaps } from "../ui/styles/classOwnership";
import { GenerationProgressPanel, statusLabel, processingPhaseLabel, formatDocumentType,
  sourceUnitHeading, formatSourceUnitMetric, formatBytes, formatProcessingDuration,
  formatApiTokens, formatDate } from "./documents/DocumentProgress";
import styles from "./DocumentsScreen.module.css";

const ownedStyleMaps = sharedClassMaps;
interface DocumentsScreenProps {
  refreshDecks: () => Promise<unknown>;
  showToast: ShowToast;
  headerSupplement?: ReactNode;
}

export function DocumentsScreen(props: DocumentsScreenProps) {
  const jobs = useDocumentJobs();
  if (!jobs) return <DocumentJobsProvider refreshDecks={props.refreshDecks} showToast={props.showToast}>
    <DocumentsScreen {...props} />
  </DocumentJobsProvider>;
  const { documents, generationProgress, clockNow, isLoading, isUploading, busyDocumentId,
    cancelingDocumentId, deletingDocumentId, guideGeneratingDocumentId, cancelingGuideDocumentId,
    errorMessage, uploadDocument, retry, generateMore, generateStudyGuide, stopStudyGuide,
    stopGeneration, openSource, deleteDocument } = jobs;
  const { headerSupplement } = props;

  return (
    <section className={styles.screen} aria-label="Documents">
      <PageHeader
        title="Documents"
        icon="document"
        compact
        actions={
          <button
            type="button"
            className={ownedClassNames(ownedStyleMaps, "primary-button")}
            disabled={
              isUploading ||
              busyDocumentId !== null ||
              deletingDocumentId !== null ||
              guideGeneratingDocumentId !== null
            }
            onClick={() => void uploadDocument()}
          >
            {isUploading ? "Processing document…" : "Upload document"}
          </button>
        }
      />

      {headerSupplement}

      <div className={styles.toolbar}>
        <span className={styles.count}>
          {documents.length} document{documents.length === 1 ? "" : "s"}
        </span>
      </div>

      {errorMessage ? (
        <div className={styles.error} role="alert">
          {errorMessage}
        </div>
      ) : null}

      {isLoading ? (
        <div className={styles.empty} aria-live="polite">
          Loading document library…
        </div>
      ) : documents.length === 0 ? (
        <div className={styles.empty}>
          <h2>No documents yet</h2>
          <p>Upload a course handout, chapter, presentation, or study packet to create learning materials.</p>
        </div>
      ) : (
        <div className={styles.list}>
          {documents.map((document) => {
            const guideGenerating = guideGeneratingDocumentId === document.id;
            const busy =
              busyDocumentId === document.id ||
              deletingDocumentId === document.id ||
              guideGenerating;
            const canceling = cancelingDocumentId === document.id;
            const cancelingGuide = cancelingGuideDocumentId === document.id;
            const live = generationProgress[document.id] ?? null;
            const canRetry =
              document.generatedDeckId === null && document.lifecycleState !== "processing";
            const canGenerateMore =
              document.generatedDeckId !== null && document.lifecycleState === "ready";
            const canGenerateGuide = document.lifecycleState !== "processing";
            const actions: ResponsiveAction[] = [
              {
                id: "view",
                label: "View",
                placement: "always-visible",
                onSelect: () => void openSource(document),
                disabled: busy,
              },
            ];
            if (document.lifecycleState === "processing") {
              actions.push({
                id: "stop-generation",
                label: canceling ? "Stopping…" : "Stop generation",
                placement: "always-visible",
                onSelect: () => void stopGeneration(document),
                disabled: canceling,
              });
            }
            if (canGenerateGuide) {
              actions.push({
                id: "generate-guide",
                label: guideGenerating ? (cancelingGuide ? "Stopping study guide…" : "Generating study guide…") : "Generate study guide",
                description: guideGenerating ? "Stop study guide generation. Saved topics remain available." : undefined,
                icon: guideGenerating ? "stop" : undefined,
                placement: "always-visible",
                variant: "primary",
                onSelect: () => void (guideGenerating ? stopStudyGuide(document) : generateStudyGuide(document)),
                disabled: guideGenerating ? cancelingGuide : busy || isUploading,
              });
            }
            if (canRetry) {
              actions.push({
                id: "retry",
                label: busyDocumentId === document.id ? "Retrying…" : "Retry generation",
                placement: "always-visible",
                variant: "primary",
                onSelect: () => void retry(document),
                disabled: busy || isUploading,
              });
            }
            if (canGenerateMore) {
              actions.push({
                id: "generate-more",
                label: busyDocumentId === document.id ? "Generating…" : "Generate more cards",
                placement: "wide-and-medium",
                variant: "primary",
                onSelect: () => void generateMore(document),
                disabled: busy || isUploading,
              });
            }
            if (document.lifecycleState !== "processing") {
              actions.push({
                id: "delete",
                label: "Delete document",
                description: "Permanently delete this managed source and document-owned derivatives.",
                placement: "wide-and-medium",
                presentation: "icon",
                icon: "delete",
                destructive: true,
                onSelect: () => void deleteDocument(document),
                disabled: busy || isUploading,
                loading: deletingDocumentId === document.id,
              });
            }
            return (
              <article key={document.id} className={styles.card}>
                <div className={styles.cardMain}>
                  <div className={styles.titleRow}>
                    <h2>{document.title}</h2>
                    <span className={styles.status} data-state={document.lifecycleState}>
                      {statusLabel(document)}
                    </span>
                    {document.lifecycleState === "processing" ? (
                      <span className={styles.processingPhase} aria-live="polite">
                        {processingPhaseLabel(document, canceling, live)}
                      </span>
                    ) : guideGenerating ? (
                      <span className={styles.processingPhase} aria-live="polite">
                        {cancelingGuide ? "Stopping study guide" : "Generating study guide"}
                      </span>
                    ) : null}
                    {document.lifecycleState !== "processing" ? (
                      <InfoButton
                        label={`View metadata for ${document.title}`}
                        title={`${document.title} metadata`}
                        description="Source and processing details for this document."
                        className={styles.metadataButton}
                      >
                        <MetadataList>
                          <MetadataItem
                            label="Format and size"
                            value={`${formatDocumentType(document.sourceFormat)} · ${formatBytes(document.byteSize)}`}
                          />
                          <MetadataItem
                            label={sourceUnitHeading(document.sourceUnitKind)}
                            value={formatSourceUnitMetric(document)}
                          />
                          <MetadataItem
                            label="Processing time"
                            value={formatProcessingDuration(document.processingDurationMs)}
                          />
                          <MetadataItem
                            label="API tokens"
                            value={formatApiTokens(document, null)}
                          />
                          <MetadataItem label="Updated" value={formatDate(document.updatedAt)} />
                          <MetadataItem label="Created" value={formatDate(document.createdAt)} />
                          <MetadataItem label="SHA-256" value={document.sha256} />
                        </MetadataList>
                      </InfoButton>
                    ) : null}
                  </div>
                  <div className={styles.sourceSummary}>
                    <span className={styles.filename}>{document.originalFilename}</span>
                    <span className={styles.cardCount}>
                      {document.generatedCardCount} card{document.generatedCardCount === 1 ? "" : "s"}
                    </span>
                  </div>
                  {document.lifecycleState === "processing" ? (
                    <GenerationProgressPanel document={document} progress={live} now={clockNow} />
                  ) : null}
                  {(document.analysisPendingItems ?? 0) > 0 ? (
                    <p role="status" className={styles.deckNote}>
                      Analysis is partial: {document.analysisPendingItems} knowledge proposals or
                      source dispositions remain unresolved. Valid material was retained.
                      Retry document generation to repair remaining items. Tutor uses saved knowledge without re-extraction.
                    </p>
                  ) : null}
                  {document.safeErrorMessage ? (
                    <p className={styles.documentError}>{document.safeErrorMessage}</p>
                  ) : null}
                  {!document.generatedDeckId &&
                  document.generatedCardCount > 0 &&
                  document.lifecycleState === "ready" ? (
                    <p className={styles.deckNote}>
                      The previous generated deck was deleted. Retry generation to create a new deck
                      from the retained document.
                    </p>
                  ) : null}
                </div>
                <ResponsiveActionGroup
                  label={`Actions for ${document.title}`}
                  actions={actions}
                  className={styles.actions}
                />
              </article>
            );
          })}
        </div>
      )}
    </section>
  );
}


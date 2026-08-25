import { useCallback, useEffect, useMemo, useState } from "react";

import type { ShowToast } from "../components/Toast";
import { getErrorCode, getErrorMessage } from "../ipc/common";
import { openDocumentSource } from "../ipc/documents";
import { publishPreparedExport, releasePreparedExport } from "../ipc/externalFiles";
import {
  cancelDocumentStudyGuideGeneration,
  discardDocumentStudyGuideOverlay,
  generateDocumentStudyGuide,
  getDocumentStudyGuide,
  getDocumentTopicCardIds,
  getDocumentTopicCardProjection,
  prepareDocumentStudyGuidePdf,
  saveDocumentStudyGuideOverlay,
  type DocumentStudyGuide,
  type DocumentStudyGuideSection,
  type StudyGuideOverlay,
  type TopicCardProjection,
} from "../ipc/studyGuides";
import { Dialog } from "../ui/overlays/Dialog";
import styles from "./StudyGuideDialog.module.css";
import { Button } from "../ui/primitives/Button";

interface StudyGuideDialogProps {
  documentId: string;
  documentTitle: string;
  showToast: ShowToast;
  onPracticeCards: (cardIds: string[]) => Promise<void>;
  onClose: () => void;
}

type EditingMode = "note" | "override";

interface EditingState {
  topicId: string | null;
  mode: EditingMode;
  value: string;
}

export function StudyGuideDialog({
  documentId,
  documentTitle,
  showToast,
  onPracticeCards,
  onClose,
}: StudyGuideDialogProps) {
  const [guide, setGuide] = useState<DocumentStudyGuide | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isGenerating, setIsGenerating] = useState(false);
  const [isCanceling, setIsCanceling] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [includeAnnotations, setIncludeAnnotations] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [editing, setEditing] = useState<EditingState | null>(null);
  const [topicCards, setTopicCards] = useState<Record<string, TopicCardProjection>>({});

  const refresh = useCallback(async () => {
    const loaded = await getDocumentStudyGuide(documentId);
    setGuide(loaded);
    setErrorMessage(null);
    return loaded;
  }, [documentId]);

  useEffect(() => {
    let active = true;
    setIsLoading(true);
    void refresh()
      .catch((error: unknown) => {
        if (active) setErrorMessage(getErrorMessage(error));
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [refresh]);

  useEffect(() => {
    if (!guide) {
      setTopicCards({});
      return;
    }
    let active = true;
    void Promise.all(
      guide.sections.map(async (section) => {
        const projection = await getDocumentTopicCardProjection(documentId, section.topicId);
        return [section.topicId, projection] as const;
      }),
    )
      .then((entries) => {
        if (active) setTopicCards(Object.fromEntries(entries));
      })
      .catch(() => {
        if (active) setTopicCards({});
      });
    return () => {
      active = false;
    };
  }, [documentId, guide]);

  const guideNote = useMemo(
    () => guide?.overlays.find((overlay) => overlay.overlayKind === "guide_note") ?? null,
    [guide],
  );

  async function generate(): Promise<void> {
    if (isGenerating || isSaving || isExporting) return;
    setIsGenerating(true);
    setErrorMessage(null);
    try {
      const generated = await generateDocumentStudyGuide(documentId);
      setGuide(generated);
      setEditing(null);
      showToast("success", guide ? "Study guide regenerated." : "Study guide created.");
    } catch (error: unknown) {
      const code = getErrorCode(error);
      if (code === "document_study_guide_canceled" || code === "ai_request_canceled") {
        setErrorMessage(null);
        showToast("info", "Study-guide generation stopped. Saved topic checkpoints can be reused on retry.");
      } else {
        const message = getErrorMessage(error);
        setErrorMessage(message);
        showToast("error", message);
      }
    } finally {
      setIsGenerating(false);
      setIsCanceling(false);
    }
  }

  async function stopGeneration(): Promise<void> {
    if (!isGenerating || isCanceling) return;
    setIsCanceling(true);
    try {
      await cancelDocumentStudyGuideGeneration(documentId);
      showToast("info", "Stopping study-guide generation…");
    } catch (error: unknown) {
      setIsCanceling(false);
      showToast("error", getErrorMessage(error));
    }
  }

  async function exportPdf(): Promise<void> {
    if (!guide || isExporting || isGenerating || isSaving) return;
    setIsExporting(true);
    let preparedExportId: string | null = null;
    try {
      const prepared = await prepareDocumentStudyGuidePdf(documentId, includeAnnotations);
      preparedExportId = prepared.preparedExport.id;
      const receipt = await publishPreparedExport(prepared.preparedExport.id);
      if (receipt) {
        showToast(
          "success",
          `Exported study guide revision ${prepared.guideRevision} with ${prepared.sectionCount} section${prepared.sectionCount === 1 ? "" : "s"}.`,
        );
      }
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      if (preparedExportId) {
        await releasePreparedExport(preparedExportId).catch(() => undefined);
      }
      setIsExporting(false);
    }
  }

  async function openSource(): Promise<void> {
    try {
      await openDocumentSource(documentId);
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function discardOverlay(overlay: StudyGuideOverlay, successMessage: string): Promise<void> {
    if (isSaving) return;
    setIsSaving(true);
    try {
      await discardDocumentStudyGuideOverlay(overlay.id, overlay.revision);
      await refresh();
      setEditing(null);
      showToast("success", successMessage);
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      setIsSaving(false);
    }
  }

  async function saveEditing(): Promise<void> {
    if (!editing || isSaving || !guide) return;
    const topic = editing.topicId
      ? guide.sections.find((section) => section.topicId === editing.topicId) ?? null
      : null;
    const existing = findEditingOverlay(guide.overlays, editing.topicId, editing.mode, guide.id);
    const isBlank = editing.value.trim().length === 0;
    setIsSaving(true);
    try {
      if (isBlank) {
        if (existing) {
          await discardDocumentStudyGuideOverlay(existing.id, existing.revision);
          await refresh();
          showToast("success", editing.mode === "override" ? "Section edit reset." : "Note removed.");
        }
        setEditing(null);
        return;
      }

      await saveDocumentStudyGuideOverlay({
        overlayId: existing?.id,
        documentId,
        guideId: editing.mode === "override" ? guide.id : undefined,
        topicId: editing.topicId ?? undefined,
        overlayKind:
          editing.mode === "override"
            ? "body_override"
            : editing.topicId
              ? "topic_note"
              : "guide_note",
        baseContentHash: editing.mode === "override" ? topic?.contentHash : undefined,
        richDoc: plainTextRichDocument(editing.value),
        plainText: editing.value,
        expectedRevision: existing?.revision,
      });
      await refresh();
      setEditing(null);
      showToast("success", editing.mode === "override" ? "Section edit saved." : "Note saved.");
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      setIsSaving(false);
    }
  }

  async function studyTopic(section: DocumentStudyGuideSection): Promise<void> {
    try {
      const cardIds = await getDocumentTopicCardIds(documentId, [section.topicId]);
      if (cardIds.length === 0) {
        showToast("info", "No flashcards are linked to this section yet.");
        return;
      }
      await onPracticeCards(cardIds);
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  function beginGuideNote(): void {
    setEditing({ topicId: null, mode: "note", value: guideNote?.plainText ?? "" });
  }

  function beginTopicNote(section: DocumentStudyGuideSection): void {
    const overlay = findEditingOverlay(guide?.overlays ?? [], section.topicId, "note", guide?.id);
    setEditing({ topicId: section.topicId, mode: "note", value: overlay?.plainText ?? "" });
  }

  function beginSectionOverride(section: DocumentStudyGuideSection): void {
    const overlay = findEditingOverlay(guide?.overlays ?? [], section.topicId, "override", guide?.id);
    setEditing({
      topicId: section.topicId,
      mode: "override",
      value: overlay?.plainText ?? section.contentText,
    });
  }

  return (
    <Dialog
      title={`Study guide · ${documentTitle}`}
      description="Source-grounded sections, learner notes, and topic-linked flashcards."
      onClose={() => onClose()}
      width="large"
      presentation="fullscreen"
      dismissible={!isGenerating && !isSaving && !isExporting}
      busy={isSaving || isExporting}
    >
      <header className={styles.header}>
        <div>
          <p className={styles.eyebrow}>Document learning artifact</p>
          <strong>{guide ? `Revision ${guide.revision}` : "Not generated yet"}</strong>
        </div>
        <div className={styles.headerActions}>
          {guide ? (
            <label className={styles.annotationToggle}>
              <input
                type="checkbox"
                checked={includeAnnotations}
                disabled={isExporting}
                onChange={(event) => setIncludeAnnotations(event.currentTarget.checked)}
              />
              Include my notes in PDF
            </label>
          ) : null}
          {guide ? (
            <Button
              type="button"
              variant="secondary"
              disabled={isExporting || isGenerating || isSaving}
              onClick={() => void exportPdf()}
            >
              {isExporting ? "Exporting…" : "Export PDF"}
            </Button>
          ) : null}
          <Button type="button" variant="secondary" onClick={() => void openSource()}>
            Open source
          </Button>
          {isGenerating ? (
            <Button
              type="button"
              variant="secondary"
              disabled={isCanceling}
              onClick={() => void stopGeneration()}
            >
              {isCanceling ? "Stopping…" : "Stop generation"}
            </Button>
          ) : (
            <Button
              type="button"
              variant={guide ? "secondary" : "primary"}
              disabled={isSaving || isExporting}
              onClick={() => void generate()}
            >
              {guide ? "Regenerate" : "Create study guide"}
            </Button>
          )}
        </div>
      </header>

      {errorMessage ? (
        <div className={styles.error} role="alert">
          {errorMessage}
        </div>
      ) : null}

      {isLoading ? (
        <div className={styles.empty} aria-live="polite">
          Loading study guide…
        </div>
      ) : !guide ? (
        <div className={styles.empty}>
          <h2>No study guide yet</h2>
          <p>
            Create a structured guide from the document’s shared learning plan. The same stable
            topic identities organize generated flashcards and future learning tools.
          </p>
        </div>
      ) : (
        <div className={styles.layout}>
          <nav className={styles.contents} aria-label="Study guide contents">
            <strong>Contents</strong>
            <a href="#guide-overview">Overview</a>
            {guide.sections.map((section) => (
              <a key={section.topicId} href={`#topic-${section.topicId}`}>
                {section.title}
              </a>
            ))}
          </nav>

          <article className={styles.guide}>
            <section id="guide-overview" className={styles.overview}>
              <h2>Overview</h2>
              <p>{guide.overview.summary}</p>
              <GuideList title="Learning objectives" values={guide.overview.learningObjectives} />
              <GuideList title="Key concepts" values={guide.overview.keyConcepts} />
              {guideNote?.plainText ? (
                <aside className={styles.personalNote}>
                  <strong>My guide note</strong>
                  <p>{guideNote.plainText}</p>
                </aside>
              ) : null}
              <div className={styles.editorActions}>
                <Button type="button" variant="secondary" onClick={beginGuideNote}>
                  {guideNote ? "Edit guide note" : "Add guide note"}
                </Button>
                {guideNote ? (
                  <Button
                    type="button"
                    variant="secondary"
                    disabled={isSaving}
                    onClick={() => void discardOverlay(guideNote, "Guide note removed.")}
                  >
                    Remove guide note
                  </Button>
                ) : null}
              </div>
            </section>

            {guide.sections.map((section) => (
              <StudyGuideSectionView
                key={section.topicId}
                section={section}
                overlays={guide.overlays}
                guideId={guide.id}
                projection={topicCards[section.topicId]}
                actionsDisabled={isSaving}
                onEditNote={() => beginTopicNote(section)}
                onEditSection={() => beginSectionOverride(section)}
                onDiscardOverlay={(overlay, message) => void discardOverlay(overlay, message)}
                onStudy={() => void studyTopic(section)}
                onOpenSource={() => void openSource()}
              />
            ))}
          </article>
        </div>
      )}

      {editing ? (
        <div className={styles.editor}>
          <label>
            <strong>{editing.mode === "override" ? "Edit generated section" : "Personal note"}</strong>
            <textarea
              value={editing.value}
              rows={10}
              onChange={(event) => {
                const value = event.currentTarget.value;
                setEditing((current) => (current ? { ...current, value } : current));
              }}
            />
          </label>
          <p>
            {editing.mode === "override"
              ? "This non-destructive override is tied to the current generated base. Regeneration keeps it only when that base is compatible; otherwise it becomes an explicit conflict for review. Clear the text and save to reset an existing section edit."
              : "Personal notes attach to the document or stable learning topic and remain available across guide regeneration. Clear the text and save to remove an existing note."}
          </p>
          <div className={styles.editorActions}>
            <Button
              type="button"
              variant="primary"
              disabled={isSaving}
              onClick={() => void saveEditing()}
            >
              {isSaving ? "Saving…" : "Save"}
            </Button>
            <Button
              type="button"
              variant="secondary"
              disabled={isSaving}
              onClick={() => setEditing(null)}
            >
              Cancel
            </Button>
          </div>
        </div>
      ) : null}
    </Dialog>
  );
}

interface StudyGuideSectionViewProps {
  section: DocumentStudyGuideSection;
  overlays: StudyGuideOverlay[];
  guideId: string;
  projection?: TopicCardProjection;
  actionsDisabled: boolean;
  onEditNote: () => void;
  onEditSection: () => void;
  onDiscardOverlay: (overlay: StudyGuideOverlay, successMessage: string) => void;
  onStudy: () => void;
  onOpenSource: () => void;
}

function StudyGuideSectionView({
  section,
  overlays,
  guideId,
  projection,
  actionsDisabled,
  onEditNote,
  onEditSection,
  onDiscardOverlay,
  onStudy,
  onOpenSource,
}: StudyGuideSectionViewProps) {
  const note = findEditingOverlay(overlays, section.topicId, "note", guideId);
  const override = findEditingOverlay(overlays, section.topicId, "override", guideId);
  const conflict = overlays.find(
    (overlay) =>
      overlay.topicId === section.topicId &&
      overlay.overlayKind === "body_override" &&
      overlay.state === "conflicted",
  );
  const sourceLabel = formatSourceLocator(section.sourceLocator);

  return (
    <section id={`topic-${section.topicId}`} className={styles.section}>
      <header className={styles.sectionHeader}>
        <div>
          <p className={styles.sourceLabel}>{sourceLabel}</p>
          <h2>{section.title}</h2>
        </div>
        <div className={styles.sectionActions}>
          <Button type="button" variant="secondary" onClick={onOpenSource}>
            Open source
          </Button>
          <Button type="button" variant="secondary" onClick={onEditNote}>
            {note ? "Edit note" : "Add note"}
          </Button>
          {note ? (
            <Button
              type="button"
              variant="secondary"
              disabled={actionsDisabled}
              onClick={() => onDiscardOverlay(note, "Topic note removed.")}
            >
              Remove note
            </Button>
          ) : null}
          <Button type="button" variant="secondary" onClick={onEditSection}>
            {override ? "Edit override" : "Edit section"}
          </Button>
          {override ? (
            <Button
              type="button"
              variant="secondary"
              disabled={actionsDisabled}
              onClick={() => onDiscardOverlay(override, "Section edit reset.")}
            >
              Reset section edit
            </Button>
          ) : null}
          <Button
            type="button"
            variant="primary"
            disabled={!projection || projection.cardCount === 0}
            onClick={onStudy}
          >
            Study linked cards{projection ? ` (${projection.cardCount})` : ""}
          </Button>
        </div>
      </header>

      {override?.state === "active" ? (
        <div className={styles.overrideText}>{renderPlainText(override.plainText)}</div>
      ) : (
        <StructuredSection section={section} />
      )}

      {note?.plainText ? (
        <aside className={styles.personalNote}>
          <strong>My note</strong>
          {renderPlainText(note.plainText)}
        </aside>
      ) : null}
      {conflict ? (
        <aside className={styles.conflict}>
          <strong>Older section edit needs review.</strong>
          <p>
            The generated base changed during regeneration, so ostudy did not silently apply the
            old override to new source-grounded content. The prior edit remains preserved below as
            a conflict.
          </p>
          <div className={styles.overrideText}>{renderPlainText(conflict.plainText)}</div>
          <Button
            type="button"
            variant="secondary"
            disabled={actionsDisabled}
            onClick={() => onDiscardOverlay(conflict, "Older section edit discarded.")}
          >
            Discard older edit
          </Button>
        </aside>
      ) : null}
    </section>
  );
}

function StructuredSection({ section }: { section: DocumentStudyGuideSection }) {
  const { content } = section;
  return (
    <div className={styles.structuredSection}>
      <p>{content.overview}</p>
      <GuideList title="Key points" values={content.keyPoints} />
      {content.explanations.map((explanation) => (
        <section key={`${explanation.heading}-${explanation.body.slice(0, 24)}`}>
          <h3>{explanation.heading}</h3>
          <p>{explanation.body}</p>
        </section>
      ))}
      {content.importantTerms.length > 0 ? (
        <section>
          <h3>Important terms</h3>
          <dl className={styles.terms}>
            {content.importantTerms.map((term) => (
              <div key={term.term}>
                <dt>{term.term}</dt>
                <dd>{term.explanation}</dd>
              </div>
            ))}
          </dl>
        </section>
      ) : null}
      <GuideList title="Process" values={content.processSteps} ordered />
      {content.comparisons.length > 0 ? (
        <section>
          <h3>Comparisons and relationships</h3>
          <div className={styles.comparisons}>
            {content.comparisons.map((comparison) => (
              <article key={`${comparison.subjectA}-${comparison.subjectB}`}>
                <strong>
                  {comparison.subjectA} and {comparison.subjectB}
                </strong>
                <p>{comparison.relationship}</p>
              </article>
            ))}
          </div>
        </section>
      ) : null}
      <GuideList title="Common misconceptions" values={content.commonMisconceptions} />
      {content.visualExplanations.map((visual) => (
        <aside
          key={`${visual.title}-${visual.explanation.slice(0, 24)}`}
          className={styles.visualNote}
        >
          <strong>{visual.title}</strong>
          <p>{visual.explanation}</p>
        </aside>
      ))}
    </div>
  );
}

function GuideList({
  title,
  values,
  ordered = false,
}: {
  title: string;
  values: string[];
  ordered?: boolean;
}) {
  if (values.length === 0) return null;
  const List = ordered ? "ol" : "ul";
  return (
    <section>
      <h3>{title}</h3>
      <List>
        {values.map((value) => (
          <li key={value}>{value}</li>
        ))}
      </List>
    </section>
  );
}

function findEditingOverlay(
  overlays: StudyGuideOverlay[],
  topicId: string | null,
  mode: EditingMode,
  guideId?: string,
): StudyGuideOverlay | null {
  const kind = mode === "override" ? "body_override" : topicId ? "topic_note" : "guide_note";
  return (
    overlays.find(
      (overlay) =>
        overlay.overlayKind === kind &&
        overlay.topicId === topicId &&
        overlay.state === "active" &&
        (mode !== "override" || overlay.guideId === guideId),
    ) ?? null
  );
}

function plainTextRichDocument(value: string): Record<string, unknown> {
  return {
    type: "doc",
    content: value
      .split(/\n{2,}/u)
      .map((paragraph) => paragraph.trim())
      .filter(Boolean)
      .map((paragraph) => ({
        type: "paragraph",
        content: [{ type: "text", text: paragraph }],
      })),
  };
}

function renderPlainText(value: string) {
  return value
    .split(/\n{2,}/u)
    .map((paragraph) => paragraph.trim())
    .filter(Boolean)
    .map((paragraph) => <p key={paragraph}>{paragraph}</p>);
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

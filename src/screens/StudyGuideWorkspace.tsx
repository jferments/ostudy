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
  prepareDocumentStudyGuidePdf,
  saveDocumentStudyGuideOverlay,
  type DocumentStudyGuide,
  type DocumentStudyGuideSection,
  type StudyGuideOverlay,
} from "../ipc/studyGuides";
import { registerPlatformBackHandler } from "../platform/backNavigation";
import { IconButton } from "../ui/icons/IconButton";
import { Button } from "../ui/primitives/Button";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import styles from "./StudyGuideWorkspace.module.css";

interface StudyGuideWorkspaceProps {
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

/** Route-owned Study Guide reader. It stays inside the application shell so
 * primary navigation remains visible and usable while a guide is open. */
export function StudyGuideWorkspace({
  documentId,
  documentTitle,
  showToast,
  onPracticeCards,
  onClose,
}: StudyGuideWorkspaceProps) {
  const [guide, setGuide] = useState<DocumentStudyGuide | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isGenerating, setIsGenerating] = useState(false);
  const [isCanceling, setIsCanceling] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [includeAnnotations, setIncludeAnnotations] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [editing, setEditing] = useState<EditingState | null>(null);

  const refresh = useCallback(async (): Promise<DocumentStudyGuide | null> => {
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

  useEffect(
    () =>
      registerPlatformBackHandler(() => {
        onClose();
        return true;
      }),
    [onClose],
  );

  const guideNote = useMemo(
    () => guide?.overlays.find((overlay) => overlay.overlayKind === "guide_note") ?? null,
    [guide],
  );
  const emphasisTerms = useMemo(
    () => guide?.overview.keyConcepts.filter((value) => value.trim().length > 2) ?? [],
    [guide],
  );

  async function generateOrStop(): Promise<void> {
    if (isGenerating) {
      if (isCanceling) return;
      setIsCanceling(true);
      try {
        await cancelDocumentStudyGuideGeneration(documentId);
        showToast("info", "Stopping study-guide generation…");
      } catch (error: unknown) {
        setIsCanceling(false);
        showToast("error", getErrorMessage(error));
      }
      return;
    }
    if (guide || isSaving || isExporting) return;
    setIsGenerating(true);
    setErrorMessage(null);
    try {
      const generated = await generateDocumentStudyGuide(documentId);
      setGuide(generated);
      setEditing(null);
      showToast("success", "Study guide created.");
    } catch (error: unknown) {
      const code = getErrorCode(error);
      if (code === "document_study_guide_canceled" || code === "ai_request_canceled") {
        setErrorMessage(null);
        showToast(
          "info",
          "Study-guide generation stopped. Saved topic checkpoints can be reused on retry.",
        );
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

  async function exportPdf(): Promise<void> {
    if (!guide || isExporting || isGenerating || isSaving) return;
    setIsExporting(true);
    let preparedExportId: string | null = null;
    try {
      const prepared = await prepareDocumentStudyGuidePdf(documentId, includeAnnotations);
      preparedExportId = prepared.preparedExport.id;
      const receipt = await publishPreparedExport(prepared.preparedExport.id);
      if (receipt) {
        const annotationSummary =
          prepared.annotationCount > 0
            ? ` and ${prepared.annotationCount} note${prepared.annotationCount === 1 ? "" : "s"}`
            : "";
        showToast(
          "success",
          `Exported study guide with ${prepared.sectionCount} section${
            prepared.sectionCount === 1 ? "" : "s"
          }${annotationSummary}.`,
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
    const existing = findEditingOverlay(
      guide.overlays,
      editing.topicId,
      editing.mode,
      guide.id,
    );
    const isBlank = editing.value.trim().length === 0;
    setIsSaving(true);
    try {
      if (isBlank) {
        if (existing) {
          await discardDocumentStudyGuideOverlay(existing.id, existing.revision);
          await refresh();
          showToast(
            "success",
            editing.mode === "override" ? "Section edit reset." : "Note removed.",
          );
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
    const overlay = findEditingOverlay(
      guide?.overlays ?? [],
      section.topicId,
      "note",
      guide?.id,
    );
    setEditing({ topicId: section.topicId, mode: "note", value: overlay?.plainText ?? "" });
  }

  function beginSectionOverride(section: DocumentStudyGuideSection): void {
    const overlay = findEditingOverlay(
      guide?.overlays ?? [],
      section.topicId,
      "override",
      guide?.id,
    );
    setEditing({
      topicId: section.topicId,
      mode: "override",
      value: overlay?.plainText ?? section.contentText,
    });
  }

  return (
    <section className={styles.workspace} aria-label={`Study guide · ${documentTitle}`}>
      <header className={styles.header}>
        <div className={styles.heading}>
          <p className={styles.eyebrow}>Study guide</p>
          <h1>{documentTitle}</h1>
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
              loading={isExporting}
              onClick={() => void exportPdf()}
            >
              Export PDF
            </Button>
          ) : null}
          <Button type="button" variant="secondary" onClick={() => void openSource()}>
            Open source
          </Button>
          {!isLoading && (!guide || isGenerating) ? (
            <button
              type="button"
              className={`${styles.generateButton} ${isGenerating ? styles.generating : ""}`.trim()}
              disabled={isSaving || isExporting || isCanceling}
              aria-label={
                isGenerating
                  ? isCanceling
                    ? "Stopping study guide generation"
                    : "Stop study guide generation"
                  : "Generate study guide"
              }
              onClick={() => void generateOrStop()}
            >
              {isGenerating ? (
                <span className={styles.stopSign} aria-hidden="true">
                  <span />
                </span>
              ) : null}
              {isCanceling
                ? "Stopping study guide…"
                : isGenerating
                  ? "Generating study guide"
                  : "Generate study guide"}
            </button>
          ) : null}
          <IconButton
            icon="close"
            iconSize={24}
            label="Close study guide"
            tooltip={false}
            className={styles.closeButton}
            disabled={isGenerating || isSaving || isExporting}
            onClick={onClose}
          />
        </div>
      </header>

      {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}

      {isLoading ? (
        <div className={styles.empty} aria-live="polite">
          Loading study guide…
        </div>
      ) : !guide ? (
        <div className={styles.empty}>
          <h2>No study guide yet</h2>
          <p>
            Generate a structured guide from the document’s shared learning plan. The same stable
            topic identities organize linked flashcards and future learning tools.
          </p>
        </div>
      ) : (
        <div className={styles.layout}>
          <nav className={styles.contents} aria-label="Study guide contents">
            <strong>Contents</strong>
            <a href="#guide-overview">Overview</a>
            {guide.sections.map((section, index) => (
              <a key={section.topicId} href={`#topic-${section.topicId}`}>
                {index + 1}. {section.title}
              </a>
            ))}
          </nav>

          <article className={styles.guide}>
            <section id="guide-overview" className={styles.overview}>
              <p className={styles.sectionNumber}>Overview</p>
              <h2>{documentTitle}</h2>
              <SemanticParagraphs text={guide.overview.summary} terms={emphasisTerms} />
              <GuideList title="Learning objectives" values={guide.overview.learningObjectives} />
              <GuideList title="Key concepts" values={guide.overview.keyConcepts} />
              {guideNote?.plainText ? (
                <aside className={styles.personalNote}>
                  <strong>My guide note</strong>
                  <p>{guideNote.plainText}</p>
                </aside>
              ) : null}
              <div className={styles.sectionActions}>
                <Button type="button" variant="secondary" onClick={beginGuideNote}>
                  {guideNote ? "Edit guide note" : "Add guide note"}
                </Button>
                {guideNote ? (
                  <Button
                    type="button"
                    variant="dangerSecondary"
                    disabled={isSaving}
                    onClick={() => void discardOverlay(guideNote, "Guide note removed.")}
                  >
                    Remove note
                  </Button>
                ) : null}
              </div>
            </section>

            {guide.sections.map((section, index) => {
              const note = findEditingOverlay(
                guide.overlays,
                section.topicId,
                "note",
                guide.id,
              );
              const override = findEditingOverlay(
                guide.overlays,
                section.topicId,
                "override",
                guide.id,
              );
              return (
                <section
                  key={section.topicId}
                  id={`topic-${section.topicId}`}
                  className={styles.section}
                >
                  <div className={styles.sectionHeading}>
                    <div>
                      <p className={styles.sectionNumber}>Section {index + 1}</p>
                      <h2>{section.title}</h2>
                    </div>
                    <Button
                      type="button"
                      variant="primary"
                      onClick={() => void studyTopic(section)}
                    >
                      Practice linked cards
                    </Button>
                  </div>
                  <SemanticParagraphs
                    text={override?.plainText ?? section.contentText}
                    terms={emphasisTerms}
                  />
                  {note?.plainText ? (
                    <aside className={styles.personalNote}>
                      <strong>My topic note</strong>
                      <p>{note.plainText}</p>
                    </aside>
                  ) : null}
                  <div className={styles.sectionActions}>
                    <Button type="button" variant="secondary" onClick={() => beginTopicNote(section)}>
                      {note ? "Edit note" : "Add note"}
                    </Button>
                    <Button
                      type="button"
                      variant="secondary"
                      onClick={() => beginSectionOverride(section)}
                    >
                      {override ? "Edit section changes" : "Edit section"}
                    </Button>
                    {override ? (
                      <Button
                        type="button"
                        variant="dangerSecondary"
                        disabled={isSaving}
                        onClick={() => void discardOverlay(override, "Section edit reset.")}
                      >
                        Reset section
                      </Button>
                    ) : null}
                  </div>
                </section>
              );
            })}
          </article>

          <aside className={styles.context} aria-label="Study guide context">
            <strong>Source and actions</strong>
            <p>{documentTitle}</p>
            <Button type="button" variant="secondary" onClick={() => void openSource()}>
              Open source document
            </Button>
            <p>
              Generated sections remain connected to the source document and linked flashcards.
              Personal notes remain stable when a guide is generated again from Documents.
            </p>
          </aside>
        </div>
      )}

      {editing ? (
        <section className={styles.editor} aria-label="Study guide editor">
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
              ? "This non-destructive edit is tied to the generated base. Clear the text and save to reset it."
              : "Personal notes attach to the document or stable topic and remain across later generation."}
          </p>
          <div className={styles.sectionActions}>
            <Button type="button" variant="primary" disabled={isSaving} onClick={() => void saveEditing()}>
              {isSaving ? "Saving…" : "Save"}
            </Button>
            <Button type="button" variant="secondary" disabled={isSaving} onClick={() => setEditing(null)}>
              Cancel
            </Button>
          </div>
        </section>
      ) : null}
    </section>
  );
}

function GuideList({ title, values }: { title: string; values: readonly string[] }) {
  if (values.length === 0) return null;
  return (
    <section className={styles.callout}>
      <h3>{title}</h3>
      <ul>
        {values.map((value, index) => (
          <li key={`${title}-${index}`}>{value}</li>
        ))}
      </ul>
    </section>
  );
}

function SemanticParagraphs({ text, terms }: { text: string; terms: readonly string[] }) {
  const paragraphs = text.split(/\n{2,}/u).map((value) => value.trim()).filter(Boolean);
  return (
    <>
      {paragraphs.map((paragraph, index) => (
        <p key={index}>{emphasizeText(paragraph, terms)}</p>
      ))}
    </>
  );
}

function emphasizeText(text: string, terms: readonly string[]) {
  const normalizedTerms = [...new Set(terms.map((value) => value.trim()).filter(Boolean))]
    .sort((left, right) => right.length - left.length)
    .slice(0, 24);
  if (normalizedTerms.length === 0) return text;
  const pattern = new RegExp(
    `(${normalizedTerms.map(escapeRegExp).join("|")})`,
    "giu",
  );
  return text.split(pattern).map((part, index) =>
    normalizedTerms.some((term) => term.localeCompare(part, undefined, { sensitivity: "accent" }) === 0) ? (
      <strong key={index}>{part}</strong>
    ) : (
      part
    ),
  );
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function findEditingOverlay(
  overlays: readonly StudyGuideOverlay[],
  topicId: string | null,
  mode: EditingMode,
  guideId?: string,
): StudyGuideOverlay | null {
  return (
    overlays.find((overlay) => {
      if ((overlay.topicId ?? null) !== topicId) return false;
      if (mode === "override") {
        return overlay.overlayKind === "body_override" && overlay.guideId === guideId;
      }
      return topicId === null
        ? overlay.overlayKind === "guide_note"
        : overlay.overlayKind === "topic_note";
    }) ?? null
  );
}

function plainTextRichDocument(value: string) {
  const paragraphs = value.split(/\n/u);
  return {
    type: "doc" as const,
    content: paragraphs.map((text) => ({
      type: "paragraph" as const,
      content: text.length > 0 ? [{ type: "text" as const, text }] : [],
    })),
  };
}
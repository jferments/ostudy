import { useCallback, useEffect, useMemo, useState } from "react";

import type { ShowToast } from "../components/Toast";
import { getErrorMessage } from "../ipc/common";
import {
  generateDocumentStudyGuide,
  getDocumentStudyGuide,
  getDocumentTopicCardIds,
  getDocumentTopicCardProjection,
  saveDocumentStudyGuideOverlay,
  type DocumentStudyGuide,
  type DocumentStudyGuideSection,
  type StudyGuideOverlay,
  type TopicCardProjection,
} from "../ipc/studyGuides";
import styles from "./StudyGuideDialog.module.css";

interface StudyGuideDialogProps {
  documentId: string;
  documentTitle: string;
  showToast: ShowToast;
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
  onClose,
}: StudyGuideDialogProps) {
  const [guide, setGuide] = useState<DocumentStudyGuide | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isGenerating, setIsGenerating] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
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
    if (isGenerating || isSaving) return;
    setIsGenerating(true);
    setErrorMessage(null);
    try {
      const generated = await generateDocumentStudyGuide(documentId);
      setGuide(generated);
      setEditing(null);
      showToast(
        "success",
        guide ? "Study guide regenerated." : "Study guide created.",
      );
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    } finally {
      setIsGenerating(false);
    }
  }

  async function saveEditing(): Promise<void> {
    if (!editing || isSaving || !guide) return;
    const topic = editing.topicId
      ? guide.sections.find((section) => section.topicId === editing.topicId) ?? null
      : null;
    const existing = findEditingOverlay(guide.overlays, editing.topicId, editing.mode, guide.id);
    setIsSaving(true);
    try {
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
      window.dispatchEvent(
        new CustomEvent("ostudy:practice-exact-cards", {
          detail: { cardIds, source: "study-guide", topicId: section.topicId },
        }),
      );
      showToast(
        "success",
        `Prepared ${cardIds.length} linked flashcard${cardIds.length === 1 ? "" : "s"} for practice.`,
      );
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
    <div className={styles.backdrop} role="presentation" onMouseDown={onClose}>
      <section
        className={styles.dialog}
        role="dialog"
        aria-modal="true"
        aria-labelledby="study-guide-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className={styles.header}>
          <div>
            <p className={styles.eyebrow}>Study guide</p>
            <h1 id="study-guide-title">{documentTitle}</h1>
          </div>
          <div className={styles.headerActions} data-print-hidden>
            {guide ? (
              <button type="button" className="secondary-button" onClick={() => window.print()}>
                Export PDF
              </button>
            ) : null}
            <button
              type="button"
              className={guide ? "secondary-button" : "primary-button"}
              disabled={isGenerating || isSaving}
              onClick={() => void generate()}
            >
              {isGenerating ? "Generating…" : guide ? "Regenerate" : "Create study guide"}
            </button>
            <button type="button" className="secondary-button" onClick={onClose}>
              Close
            </button>
          </div>
        </header>

        {errorMessage ? (
          <div className={styles.error} role="alert">
            {errorMessage}
          </div>
        ) : null}

        {isLoading ? (
          <div className={styles.empty}>Loading study guide…</div>
        ) : !guide ? (
          <div className={styles.empty}>
            <h2>No study guide yet</h2>
            <p>
              Create a structured guide from the document’s shared learning plan. The same topic
              identities can later organize linked flashcards, tests, and tutoring.
            </p>
          </div>
        ) : (
          <div className={styles.layout}>
            <nav className={styles.contents} aria-label="Study guide contents" data-print-hidden>
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
                <button
                  type="button"
                  className="secondary-button"
                  data-print-hidden
                  onClick={beginGuideNote}
                >
                  {guideNote ? "Edit guide note" : "Add guide note"}
                </button>
              </section>

              {guide.sections.map((section) => (
                <StudyGuideSectionView
                  key={section.topicId}
                  section={section}
                  overlays={guide.overlays}
                  guideId={guide.id}
                  projection={topicCards[section.topicId]}
                  onEditNote={() => beginTopicNote(section)}
                  onEditSection={() => beginSectionOverride(section)}
                  onStudy={() => void studyTopic(section)}
                />
              ))}
            </article>
          </div>
        )}

        {editing ? (
          <div className={styles.editor} data-print-hidden>
            <label>
              <strong>{editing.mode === "override" ? "Edit generated section" : "Personal note"}</strong>
              <textarea
                value={editing.value}
                rows={10}
                onChange={(event) =>
                  setEditing((current) =>
                    current ? { ...current, value: event.currentTarget.value } : current,
                  )
                }
              />
            </label>
            <p>
              {editing.mode === "override"
                ? "This is a non-destructive override of the current generated revision. Regeneration preserves it as a conflict for review instead of silently applying it to changed source text."
                : "Personal notes are attached to the stable document topic and remain available after regeneration."}
            </p>
            <div className={styles.editorActions}>
              <button
                type="button"
                className="primary-button"
                disabled={isSaving}
                onClick={() => void saveEditing()}
              >
                {isSaving ? "Saving…" : "Save"}
              </button>
              <button
                type="button"
                className="secondary-button"
                disabled={isSaving}
                onClick={() => setEditing(null)}
              >
                Cancel
              </button>
            </div>
          </div>
        ) : null}
      </section>
    </div>
  );
}

interface StudyGuideSectionViewProps {
  section: DocumentStudyGuideSection;
  overlays: StudyGuideOverlay[];
  guideId: string;
  projection?: TopicCardProjection;
  onEditNote: () => void;
  onEditSection: () => void;
  onStudy: () => void;
}

function StudyGuideSectionView({
  section,
  overlays,
  guideId,
  projection,
  onEditNote,
  onEditSection,
  onStudy,
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
        <div className={styles.sectionActions} data-print-hidden>
          <button type="button" className="secondary-button" onClick={onEditNote}>
            {note ? "Edit note" : "Add note"}
          </button>
          <button type="button" className="secondary-button" onClick={onEditSection}>
            {override ? "Edit override" : "Edit section"}
          </button>
          <button
            type="button"
            className="primary-button"
            disabled={!projection || projection.cardCount === 0}
            onClick={onStudy}
          >
            Study linked cards{projection ? ` (${projection.cardCount})` : ""}
          </button>
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
        <aside className={styles.conflict} data-print-hidden>
          A section edit from an older generated revision is preserved for review. Open the section
          editor to reconcile it with the current source-grounded content.
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
        <aside key={`${visual.title}-${visual.explanation.slice(0, 24)}`} className={styles.visualNote}>
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

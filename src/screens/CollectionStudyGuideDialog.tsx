import { useCallback, useEffect, useMemo, useState } from "react";

import { StudyMarkdown } from "../components/StudyMarkdown";
import type { ShowToast } from "../components/Toast";
import { getErrorMessage } from "../ipc/common";
import { publishPreparedExport, releasePreparedExport } from "../ipc/externalFiles";
import { openDocumentSource } from "../ipc/documents";
import {
  discardCollectionStudyGuideOverlay,
  getCollectionStudyGuide,
  prepareCollectionStudyGuidePdf,
  saveCollectionStudyGuideOverlay,
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
import { Checkbox, Textarea } from "../ui/primitives/FormControls";
import { Button } from "../ui/primitives/Button";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { guideReaderStyles as styles } from "./documents/presentation";
import { StudyGuideStructuredContent } from "./StudyGuideDialog";

type CollectionEditingMode = "note" | "override";
interface CollectionEditingState { blockKey: string | null; mode: CollectionEditingMode; value: string; }

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
  const [isSaving, setIsSaving] = useState(false);
  const [includeAnnotations, setIncludeAnnotations] = useState(true);
  const [editing, setEditing] = useState<CollectionEditingState | null>(null);
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

  async function discardOverlay(overlay: CollectionStudyGuideOverlay, successMessage: string): Promise<void> {
    if (isSaving) return;
    setIsSaving(true);
    try {
      await discardCollectionStudyGuideOverlay(collectionId, overlay.id, overlay.revision);
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
    if (!editing || !guide || isSaving) return;
    const section = editing.blockKey
      ? guide.content.sections.find((candidate) => candidate.blockKey === editing.blockKey) ?? null
      : null;
    const existing = findEditingOverlay(guide.overlays, editing.blockKey, editing.mode);
    const blank = editing.value.trim().length === 0;
    setIsSaving(true);
    try {
      if (blank) {
        if (existing) {
          await discardCollectionStudyGuideOverlay(collectionId, existing.id, existing.revision);
          await refresh();
          showToast("success", editing.mode === "override" ? "Section edit reset." : "Note removed.");
        }
        setEditing(null);
        return;
      }
      await saveCollectionStudyGuideOverlay({
        overlayId: existing?.id,
        collectionId,
        guideId: guide.id,
        blockKey: editing.blockKey ?? undefined,
        overlayKind: editing.mode === "override"
          ? "body_override"
          : editing.blockKey ? "topic_note" : "guide_note",
        baseContentHash: editing.mode === "override" ? section?.contentHash : undefined,
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

  async function exportPdf(): Promise<void> {
    if (!guide || isExporting) return;
    setIsExporting(true);
    let preparedExportId: string | null = null;
    try {
      const prepared = await prepareCollectionStudyGuidePdf(collectionId, includeAnnotations);
      preparedExportId = prepared.preparedExport.id;
      if (prepared.mathSourceFallback && !await confirm({
        title: "Export math as source notation?",
        description: "Some equations use notation outside the native PDF math subset and will remain readable LaTeX source. The study guide in oStudy keeps its full typeset math.",
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
      dismissible={!isExporting && !isSaving}
      busy={isExporting || isSaving}
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
              <div className={styles.editorActions}>
                <OverflowMenu
                  label="Guide actions"
                  items={[
                    {
                      id: "guide-note",
                      label: guideNote ? "Edit guide note" : "Add guide note",
                      disabled: isSaving || editing !== null,
                      onSelect: () => setEditing({
                        blockKey: null,
                        mode: "note",
                        value: guideNote?.plainText ?? "",
                      }),
                    },
                    ...(guideNote ? [{
                      id: "remove-guide-note",
                      label: "Remove guide note",
                      destructive: true,
                      disabled: isSaving,
                      onSelect: () => void discardOverlay(guideNote, "Guide note removed."),
                    }] : []),
                  ]}
                />
              </div>
            </section>
            {guide.content.sections.map((section, index) => (
              <CollectionSection
                key={section.blockKey}
                section={section}
                index={index}
                collectionId={collectionId}
                overlays={guide.overlays}
                actionsDisabled={isSaving || editing !== null}
                onEditNote={() => setEditing({
                  blockKey: section.blockKey,
                  mode: "note",
                  value: findEditingOverlay(guide.overlays, section.blockKey, "note")?.plainText ?? "",
                })}
                onEditSection={() => setEditing({
                  blockKey: section.blockKey,
                  mode: "override",
                  value: findEditingOverlay(guide.overlays, section.blockKey, "override")?.plainText
                    ?? semanticContentPlainText(section.content),
                })}
                onDiscardOverlay={(overlay, message) => void discardOverlay(overlay, message)}
                onOpenSource={() => void openSource(section)}
              />
            ))}
          </article>
        </div>
      )}
    {editing ? <div className={styles.editor}>
      <label>
        <strong>{editing.mode === "override" ? "Edit generated section" : "Personal note"}</strong>
        <Textarea
          value={editing.value}
          rows={10}
          onChange={(event) => {
            const value = event.currentTarget.value;
            setEditing((current) => current ? { ...current, value } : current);
          }}
        />
      </label>
      <p>
        {editing.mode === "override"
          ? "This non-destructive edit is tied to the current collection-guide section. Clear the text and save to reset it."
          : "Personal notes remain learner-owned. Clear the text and save to remove an existing note."}
      </p>
      <div className={styles.editorActions}>
        <Button variant="primary" disabled={isSaving} onClick={() => void saveEditing()}>
          {isSaving ? "Saving…" : "Save"}
        </Button>
        <Button variant="secondary" disabled={isSaving} onClick={() => setEditing(null)}>Cancel</Button>
      </div>
    </div> : null}
    </InlineWorkspace>
  );
}

function CollectionSection({
  section,
  index,
  collectionId,
  overlays,
  actionsDisabled,
  onEditNote,
  onEditSection,
  onDiscardOverlay,
  onOpenSource,
}: {
  section: CollectionGuideSection;
  index: number;
  collectionId: string;
  overlays: CollectionStudyGuideOverlay[];
  actionsDisabled: boolean;
  onEditNote: () => void;
  onEditSection: () => void;
  onDiscardOverlay: (overlay: CollectionStudyGuideOverlay, message: string) => void;
  onOpenSource: () => void;
}) {
  const tutor = useOptionalTutor();
  const note = findEditingOverlay(overlays, section.blockKey, "note");
  const override = findEditingOverlay(overlays, section.blockKey, "override");
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
    {
      id: "note",
      label: note ? "Edit note" : "Add note",
      disabled: actionsDisabled,
      onSelect: onEditNote,
    },
    {
      id: "edit",
      label: override ? "Edit section override" : "Edit section",
      disabled: actionsDisabled,
      onSelect: onEditSection,
    },
    ...(note ? [{
      id: "remove-note",
      label: "Remove note",
      destructive: true,
      disabled: actionsDisabled,
      onSelect: () => onDiscardOverlay(note, "Section note removed."),
    }] : []),
    ...(override ? [{
      id: "reset-section",
      label: "Reset section edit",
      destructive: true,
      disabled: actionsDisabled,
      onSelect: () => onDiscardOverlay(override, "Section edit reset."),
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
        <StudyGuideStructuredContent
          content={section.content}
          figureSource={section.sourceDocumentId && section.sourceLocator
            ? { documentId: section.sourceDocumentId, locator: section.sourceLocator }
            : null}
        />
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

function findEditingOverlay(
  overlays: CollectionStudyGuideOverlay[],
  blockKey: string | null,
  mode: CollectionEditingMode,
): CollectionStudyGuideOverlay | null {
  const kind = mode === "override" ? "body_override" : blockKey ? "topic_note" : "guide_note";
  return overlays.find((overlay) =>
    overlay.blockKey === blockKey
    && overlay.overlayKind === kind
    && overlay.state === "active") ?? null;
}

function semanticContentPlainText(content: CollectionGuideSection["content"]): string {
  const lines = [content.overview];
  lines.push(...content.keyPoints);
  for (const explanation of content.explanations) lines.push(explanation.heading, explanation.body);
  for (const term of content.importantTerms) lines.push(`${term.term}: ${term.explanation}`);
  lines.push(...content.processSteps);
  for (const comparison of content.comparisons) {
    lines.push(`${comparison.subjectA} / ${comparison.subjectB}: ${comparison.relationship}`);
  }
  lines.push(...content.commonMisconceptions);
  for (const callout of content.callouts) lines.push(callout.title, callout.body);
  for (const table of content.tables) {
    lines.push(table.title, table.columns.join(" | "));
    lines.push(...table.rows.map((row) => row.join(" | ")));
    if (table.caption) lines.push(table.caption);
  }
  for (const example of content.workedExamples) {
    lines.push(example.title, example.prompt, ...example.steps, example.answer);
    if (example.takeaway) lines.push(example.takeaway);
  }
  for (const check of content.retrievalChecks) lines.push(check.prompt, check.answer);
  for (const visual of content.visualExplanations) lines.push(visual.title, visual.explanation);
  return lines.filter((line) => line.trim().length > 0).join("\n\n");
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

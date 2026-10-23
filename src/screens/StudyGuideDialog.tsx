import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ShowToast } from "../components/Toast";
import { getErrorCode, getErrorMessage } from "../ipc/common";
import { openDocumentSource } from "../ipc/documents";
import {
  cancelDocumentStudyGuideGeneration, discardDocumentStudyGuideOverlay,
  generateDocumentStudyGuide, getDocumentStudyGuide, getDocumentTopicCardIds,
  getDocumentTopicCardProjection, openDocumentStudyGuidePdf, saveDocumentStudyGuideOverlay,
  type DocumentStudyGuide, type DocumentStudyGuideSection, type StudyGuideOverlay, type TopicCardProjection,
} from "../ipc/studyGuides";
import { getStudyGuideReading, openStudyGuideSection, setStudyGuideSectionRead, type StudyGuideReading } from "../ipc/studyGuideReading";
import { TutorEntryButton } from "./tutor/TutorEntryButton";
import { useOptionalTutor } from "./tutor/TutorContext";
import { Icon } from "../ui/icons/Icon";
import { IconButton } from "../ui/icons/IconButton";
import { useLayoutMode } from "../ui/layout/LayoutModeProvider";
import { InlineWorkspace, Sheet } from "../ui/overlays/Dialog";
import { OverflowMenu, type OverflowMenuItem } from "../ui/overlays/OverflowMenu";
import { useConfirm } from "../ui/overlays/overlayContext";
import { EmptyState, LoadingState } from "../ui/primitives/EmptyState";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { Button } from "../ui/primitives/Button";
import { Progress } from "../ui/primitives/Progress";
import { GuideTextDialog } from "./study-guides/GuideDialogs";
import { exportGuidePdf } from "./study-guides/exportGuidePdf";
import { type GuideDestination, GuideOutline, guidePageId } from "./study-guides/GuideOutline";
import { GuideOverviewPage, type GuidePageLink, GuideSectionPage } from "./study-guides/GuideReaderPages";
import {
  type EditingMode, findEditingOverlay, type GuideSectionEntry, type GuideSourceSummary, guideReadingOrder,
  plainTextRichDocument, sectionBlocks,
} from "./study-guides/guideModel";
import type { CheckProgressMap } from "./study-guides/StudyGuideContent";
import { GuideCanvas, type GuideCanvasLeave, type GuideCanvasLinkActions } from "./study-guides/GuideCanvas";
import {
  GUIDE_PAGE_STYLES, type GuidePageStyle, readGuidePageStyle, saveGuidePageStyle,
} from "./study-guides/useDesignedSectionPage";

interface StudyGuideDialogProps extends GuideCanvasLinkActions {
  documentId: string; documentTitle: string;
  /** What the library knows about the source file, for its kind badge and length. */
  source?: GuideSourceSummary | null;
  showToast: ShowToast;
  onPracticeCards: (cardIds: string[]) => Promise<void>; onClose: () => void;
}
interface EditingState { topicId: string | null; mode: EditingMode; initialValue: string; }

/**
 * The reader for one document's study guide. In a page theme it is the guide's PDF pages on one
 * canvas to write on, with the contents beside them ({@link GuideCanvas}); as plain text it is the
 * Chapter reader, an overview page and one page per section, with the outline beside them on wide
 * screens and in a Contents sheet on narrower ones.
 */
export function StudyGuideDialog({ documentId, documentTitle, source = null, showToast, onPracticeCards, onClose, ...links }: StudyGuideDialogProps) {
  const confirm = useConfirm();
  const tutor = useOptionalTutor();
  const { layoutMode } = useLayoutMode();
  const [guide, setGuide] = useState<DocumentStudyGuide | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [loadFailed, setLoadFailed] = useState(false);
  const [isGenerating, setIsGenerating] = useState(false);
  const [isCanceling, setIsCanceling] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [isOpeningPdf, setIsOpeningPdf] = useState(false);
  const [isRemaking, setIsRemaking] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [editing, setEditing] = useState<EditingState | null>(null);
  const [pageStyle, setPageStyle] = useState<GuidePageStyle>(readGuidePageStyle);
  /** While the canvas is open: saves its pending marks, then does what is asked. */
  const canvasLeave = useRef<GuideCanvasLeave | null>(null);
  const [contentsOpen, setContentsOpen] = useState(false);
  const [topicCards, setTopicCards] = useState<Record<string, TopicCardProjection>>({});
  const [currentTopicId, setCurrentTopicId] = useState<string | null>(null);
  // Check-yourself answers last while the guide is open; nothing is stored.
  const [checkProgress, setCheckProgress] = useState<Record<string, CheckProgressMap>>({});
  const [navigation, setNavigation] = useState<{ scrollId: string; focusId: string; sequence: number } | null>(null);
  // Which sections are read and where the learner stopped. Reading is not a review: it never
  // changes a card's schedule, and a failed save only leaves the marks as they were.
  const [reading, setReading] = useState<StudyGuideReading | null>(null);
  const resumed = useRef(false);
  const loadSequence = useRef(0);
  const refresh = useCallback(async () => {
    const loaded = await getDocumentStudyGuide(documentId);
    setGuide(loaded); setErrorMessage(null); setLoadFailed(false);
    const stored = loaded ? await getStudyGuideReading(documentId).catch(() => null) : null;
    setReading(stored?.guideId === loaded?.id ? stored : null);
    // The guide reopens on the section the learner last opened, once per visit.
    if (!resumed.current && loaded && stored?.lastTopicId && loaded.sections.some((section) => section.topicId === stored.lastTopicId)) {
      setCurrentTopicId(stored.lastTopicId);
    }
    resumed.current = true;
    return loaded;
  }, [documentId]);
  const load = useCallback(() => {
    const sequence = ++loadSequence.current;
    setIsLoading(true);
    void refresh().catch((error: unknown) => {
      if (sequence !== loadSequence.current) return;
      setErrorMessage(getErrorMessage(error)); setLoadFailed(true);
    }).finally(() => { if (sequence === loadSequence.current) setIsLoading(false); });
  }, [refresh]);
  useEffect(() => {
    load();
    return () => { loadSequence.current += 1; };
  }, [load]);
  useEffect(() => {
    if (!guide) { setTopicCards({}); return; }
    let active = true;
    const topicIds = guide.sections.map((section) => section.topicId);
    void Promise.allSettled(topicIds.map((topicId) => getDocumentTopicCardProjection(documentId, topicId))).then((results) => {
      if (!active) return;
      setTopicCards(Object.fromEntries(results.flatMap((result, index) => {
        const topicId = topicIds[index];
        return result.status === "fulfilled" && topicId ? [[topicId, result.value] as const] : [];
      })));
    });
    return () => { active = false; };
  }, [documentId, guide]);
  // A page change scrolls back to the top of the page and focuses its title; a block jump scrolls to
  // its heading and focuses that. The focus waits a microtask, until a closing Contents sheet has
  // returned focus to its button.
  useEffect(() => {
    if (!navigation) return;
    queueMicrotask(() => {
      document.getElementById(navigation.scrollId)?.scrollIntoView?.({ block: "start" });
      document.getElementById(navigation.focusId)?.focus({ preventScroll: true });
    });
  }, [navigation]);

  // Opening a section is where the guide reopens next time.
  const guideId = guide?.id ?? null;
  useEffect(() => {
    if (!guideId || !currentTopicId) return;
    let active = true;
    void openStudyGuideSection(documentId, guideId, currentTopicId)
      .then((saved) => { if (active) setReading(saved); })
      .catch(() => undefined);
    return () => { active = false; };
  }, [documentId, guideId, currentTopicId]);

  const order = useMemo(() => guide ? guideReadingOrder(guide) : null, [guide]);
  const readTopicIds = useMemo(() => new Set(reading?.readTopicIds ?? []), [reading]);
  const guideNote = useMemo(() => guide ? findEditingOverlay(guide.overlays, null, "note") : null, [guide]);
  const guideTitle = guide?.editorial?.title?.trim() || documentTitle;
  const currentEntry = order?.entries.find((entry) => entry.section.topicId === currentTopicId) ?? null;
  const currentOverride = guide && currentEntry ? findEditingOverlay(guide.overlays, currentEntry.section.topicId, "override", guide.id) : null;
  const blocks = currentEntry && !currentOverride ? sectionBlocks(currentEntry.section.content) : [];
  const busy = isSaving || isExporting;
  const actionsDisabled = isSaving || editing !== null;

  function navigate(destination: GuideDestination): void {
    setContentsOpen(false);
    // Moving on to a later section counts the one just read.
    if (destination.kind === "page" && destination.topicId !== null && currentEntry && order) {
      const from = order.entries.indexOf(currentEntry);
      const to = order.entries.findIndex((entry) => entry.section.topicId === destination.topicId);
      if (to > from && !readTopicIds.has(currentEntry.section.topicId)) void setSectionRead(currentEntry.section.topicId, true);
    }
    const target = destination.kind === "page"
      ? { scrollId: guidePageId(destination.topicId), focusId: `${guidePageId(destination.topicId)}-title` }
      : { scrollId: destination.id, focusId: destination.id };
    if (destination.kind === "page") setCurrentTopicId(destination.topicId);
    setNavigation((current) => ({ ...target, sequence: (current?.sequence ?? 0) + 1 }));
  }
  async function setSectionRead(topicId: string, read: boolean): Promise<void> {
    if (!guide) return;
    try { setReading(await setStudyGuideSectionRead(documentId, guide.id, topicId, read)); }
    catch (error: unknown) { showToast("error", getErrorMessage(error)); }
  }
  async function generate(): Promise<void> {
    if (guide || isGenerating || isSaving || isExporting) return;
    setIsGenerating(true); setErrorMessage(null);
    try {
      const generated = await generateDocumentStudyGuide(documentId);
      setGuide(generated); setLoadFailed(false); setEditing(null); setCurrentTopicId(null);
      setReading(await getStudyGuideReading(documentId).catch(() => null));
      showToast("success", "Study guide created.");
    } catch (error: unknown) {
      const code = getErrorCode(error);
      if (code === "document_study_guide_canceled" || code === "ai_request_canceled") {
        setErrorMessage(null);
        showToast("info", "Study-guide generation stopped. Saved topic checkpoints can be reused on retry.");
      } else {
        const message = getErrorMessage(error); setErrorMessage(message); showToast("error", message);
      }
    } finally { setIsGenerating(false); setIsCanceling(false); }
  }
  async function stopGeneration(): Promise<void> {
    if (!isGenerating || isCanceling) return;
    setIsCanceling(true);
    try { await cancelDocumentStudyGuideGeneration(documentId); showToast("info", "Stopping study-guide generation…"); }
    catch (error: unknown) { setIsCanceling(false); showToast("error", getErrorMessage(error)); }
  }
  async function exportPdf(): Promise<void> {
    if (!guide || isExporting || isGenerating || isSaving) return;
    setIsExporting(true);
    try { await exportGuidePdf({ kind: "document", id: documentId }, confirm, showToast); }
    finally { setIsExporting(false); }
  }
  /** Opens the guide in the system's PDF reader, set in the chosen theme with the learner's notes. */
  async function openInPdfReader(): Promise<void> {
    if (!guide || isOpeningPdf) return;
    setIsOpeningPdf(true);
    try { await openDocumentStudyGuidePdf(documentId, pageStyle === "plain" ? "navy" : pageStyle); }
    catch (error: unknown) { showToast("error", getErrorMessage(error)); }
    finally { setIsOpeningPdf(false); }
  }
  /** Writes a new revision of the guide; the current one stays open until it is saved. */
  async function remake(): Promise<void> {
    if (!guide || isRemaking || isGenerating) return;
    setIsRemaking(true);
    try {
      const remade = await generateDocumentStudyGuide(documentId);
      await refresh();
      showToast("success", `Study guide revision ${remade.revision} made.`);
    } catch (error: unknown) {
      const code = getErrorCode(error);
      if (code === "document_study_guide_canceled" || code === "ai_request_canceled") {
        showToast("info", "Study guide remake stopped. Saved section progress can be reused.");
      } else showToast("error", `Study guide remake failed: ${getErrorMessage(error)}`);
    } finally { setIsRemaking(false); }
  }
  /** Leaves the reader, saving the canvas's marks first while it is open. */
  function close(): void {
    const leave = canvasLeave.current;
    if (leave) void leave(onClose);
    else onClose();
  }
  async function openSource(): Promise<void> {
    try { await openDocumentSource(documentId); } catch (error: unknown) { showToast("error", getErrorMessage(error)); }
  }
  async function discardOverlay(overlay: StudyGuideOverlay, successMessage: string): Promise<void> {
    if (isSaving) return;
    setIsSaving(true);
    try {
      await discardDocumentStudyGuideOverlay(overlay.id, overlay.revision);
      await refresh(); setEditing(null); showToast("success", successMessage);
    } catch (error: unknown) { showToast("error", getErrorMessage(error)); }
    finally { setIsSaving(false); }
  }
  async function saveEditing(value: string): Promise<void> {
    if (!editing || isSaving || !guide) return;
    const topic = editing.topicId ? guide.sections.find((section) => section.topicId === editing.topicId) ?? null : null;
    const existing = findEditingOverlay(guide.overlays, editing.topicId, editing.mode, guide.id);
    const isBlank = value.trim().length === 0;
    setIsSaving(true);
    try {
      if (isBlank) {
        if (existing) {
          await discardDocumentStudyGuideOverlay(existing.id, existing.revision);
          await refresh(); showToast("success", editing.mode === "override" ? "Section edit reset." : "Note removed.");
        }
        setEditing(null); return;
      }
      await saveDocumentStudyGuideOverlay({
        overlayId: existing?.id, documentId, guideId: editing.mode === "override" ? guide.id : undefined,
        topicId: editing.topicId ?? undefined,
        overlayKind: editing.mode === "override" ? "body_override" : editing.topicId ? "topic_note" : "guide_note",
        baseContentHash: editing.mode === "override" ? topic?.contentHash : undefined,
        richDoc: plainTextRichDocument(value), plainText: value, expectedRevision: existing?.revision,
      });
      await refresh(); setEditing(null); showToast("success", editing.mode === "override" ? "Section edit saved." : "Note saved.");
    } catch (error: unknown) { showToast("error", getErrorMessage(error)); }
    finally { setIsSaving(false); }
  }
  async function studyTopic(section: DocumentStudyGuideSection): Promise<void> {
    try {
      const cardIds = await getDocumentTopicCardIds(documentId, [section.topicId]);
      if (cardIds.length === 0) { showToast("info", "No flashcards are linked to this section yet."); return; }
      const practice = () => onPracticeCards(cardIds);
      // Practice leaves the guide, so the canvas saves its marks first.
      const leave = canvasLeave.current;
      if (leave) await leave(practice);
      else await practice();
    } catch (error: unknown) { showToast("error", getErrorMessage(error)); }
  }
  function beginGuideNote(): void { setEditing({ topicId: null, mode: "note", initialValue: guideNote?.plainText ?? "" }); }
  function beginTopicNote(section: DocumentStudyGuideSection): void {
    const overlay = findEditingOverlay(guide?.overlays ?? [], section.topicId, "note", guide?.id);
    setEditing({ topicId: section.topicId, mode: "note", initialValue: overlay?.plainText ?? "" });
  }
  function beginSectionOverride(section: DocumentStudyGuideSection): void {
    const overlay = findEditingOverlay(guide?.overlays ?? [], section.topicId, "override", guide?.id);
    setEditing({ topicId: section.topicId, mode: "override", initialValue: overlay?.plainText ?? section.contentText });
  }

  function choosePageStyle(style: GuidePageStyle): void {
    const choose = () => {
      setPageStyle(style);
      saveGuidePageStyle(style);
    };
    // A new theme lays the pages out again, so the marks are saved first.
    const leave = canvasLeave.current;
    if (leave) void leave(choose);
    else choose();
  }
  const pageStyleItems: OverflowMenuItem[] = GUIDE_PAGE_STYLES.map((style) => ({
    id: `style-${style.value}`, label: style.value === "plain" ? "Plain text" : style.name, checked: style.value === pageStyle,
    onSelect: () => choosePageStyle(style.value),
  }));
  const pageStyleName = pageStyle === "plain" ? "Plain text" : GUIDE_PAGE_STYLES.find((style) => style.value === pageStyle)?.name ?? "Navy";
  const guideMenuItems: OverflowMenuItem[] = guide ? [
    // A phone's bar has no room for the page menu, so its styles join this one.
    ...(layoutMode === "compact" ? pageStyleItems.map((item) => ({ ...item, label: item.id === "style-plain" ? "Read as plain text" : `${item.label} pages` })) : []),
    { id: "export", label: isExporting ? "Saving PDF…" : "Save a copy as PDF…", disabled: isExporting || isGenerating || isSaving,
      onSelect: () => void exportPdf() },
    { id: "guide-note", label: guideNote ? "Edit guide note" : "Add guide note", disabled: actionsDisabled, onSelect: beginGuideNote },
    { id: "remake", label: isRemaking ? "Remaking the study guide…" : "Remake the study guide", disabled: isRemaking || isGenerating || busy,
      onSelect: () => void remake() },
    ...(guideNote ? [{ id: "remove-guide-note", label: "Remove guide note", destructive: true, disabled: actionsDisabled,
      onSelect: () => void discardOverlay(guideNote, "Guide note removed.") }] : []),
  ] : [];
  /** The canvas scrolled to a section; scrolling on past one counts it read, as Next does. */
  function scrolledTo(topicId: string): void {
    if (currentEntry && order && topicId !== currentEntry.section.topicId) {
      const from = order.entries.indexOf(currentEntry);
      const to = order.entries.findIndex((entry) => entry.section.topicId === topicId);
      if (to > from && !readTopicIds.has(currentEntry.section.topicId)) void setSectionRead(currentEntry.section.topicId, true);
    }
    setCurrentTopicId(topicId);
  }
  /** A section's actions in the canvas's contents. */
  function sectionMenu(topicId: string): OverflowMenuItem[] {
    const section = guide?.sections.find((item) => item.topicId === topicId) ?? null;
    if (!guide || !section) return [];
    const override = findEditingOverlay(guide.overlays, topicId, "override", guide.id);
    const read = readTopicIds.has(topicId);
    const title = order?.entries.find((entry) => entry.section.topicId === topicId)?.title ?? section.title;
    return [
      { id: "practice", label: "Practice this section's flashcards", onSelect: () => void studyTopic(section) },
      ...(tutor ? [{ id: "tutor", label: "Ask Tutor about this section", disabled: actionsDisabled || tutor.busy || tutor.pending !== null,
        onSelect: () => tutor.openContext({ title, scope: { kind: "guide", id: documentId },
          focus: { kind: "guide", documentId, guideId: guide.id, topicId, contentHash: section.contentHash }, mode: "explain" }) }] : []),
      { id: "read", label: read ? "Mark as unread" : "Mark as read", onSelect: () => void setSectionRead(topicId, !read) },
      { id: "edit", label: "Edit or rewrite this section", disabled: actionsDisabled, onSelect: () => beginSectionOverride(section) },
      ...(override ? [{ id: "reset-section", label: "Reset section edit", destructive: true, disabled: actionsDisabled,
        onSelect: () => void discardOverlay(override, "Section edit reset.") }] : []),
    ];
  }
  const readCount = order ? order.entries.filter((entry) => readTopicIds.has(entry.section.topicId)).length : 0;
  const topBarActions = guide ? <>
    {order && order.entries.length > 0 && layoutMode !== "compact" ? (
      <div className="tw:flex tw:items-center tw:gap-2 tw:text-small tw:tabular-nums tw:text-muted-foreground">
        <Progress value={readCount} max={order.entries.length} size="small" className="tw:w-[clamp(5rem,10vw,10rem)]"
          label="Sections read" valueText={`${readCount} of ${order.entries.length} sections read`} />
        <span aria-hidden="true">{readCount} of {order.entries.length}</span>
      </div>
    ) : null}
    {layoutMode === "compact" ? (
      <IconButton icon="contents" label="Contents" tooltip={false} className="tw:size-11" aria-haspopup="dialog" aria-expanded={contentsOpen}
        onClick={() => setContentsOpen(true)} />
    ) : layoutMode === "medium" ? (
      <Button variant="ghost" aria-haspopup="dialog" aria-expanded={contentsOpen} onClick={() => setContentsOpen(true)}>
        <Icon name="contents" size={18} />Contents
      </Button>
    ) : null}
    {/* Wider bars name the action as the mockups do; a phone's bar keeps to quiet icons. */}
    <TutorEntryButton showLabel={layoutMode !== "compact"} variant={layoutMode === "compact" ? "ghost" : "secondary"}
      className={layoutMode === "compact" ? "tw:size-11" : undefined} disabled={isGenerating || busy || editing !== null}
      target={{ title: guideTitle, scope: { kind: "guide", id: documentId },
        focus: { kind: "guide", documentId, guideId: guide.id, topicId: null, contentHash: null }, mode: "explain" }} />
    {layoutMode !== "compact" ? (
      <OverflowMenu label={`Page: ${pageStyleName}`} triggerPresentation="text" items={pageStyleItems} />
    ) : null}
    {layoutMode === "compact" ? (
      <IconButton icon="document" label="Open in PDF reader" tooltip={false} className="tw:size-11"
        disabled={isOpeningPdf || isGenerating || isSaving} onClick={() => void openInPdfReader()} />
    ) : (
      <Button variant="secondary" loading={isOpeningPdf} disabled={isGenerating || isSaving} onClick={() => void openInPdfReader()}>
        <Icon name="document" size={18} />Open in PDF reader
      </Button>
    )}
    <OverflowMenu label="Study guide actions" items={guideMenuItems} className="tw:compact:size-11" />
  </> : null;

  // The canvas lays out again when the guide's text does: a new revision, or a section edit.
  const contentKey = guide ? [guide.id, ...guide.overlays
    .filter((overlay) => overlay.overlayKind === "body_override" && overlay.state === "active")
    .map((overlay) => `${overlay.id}.${overlay.revision}`)].join(":") : "";
  const onCanvas = guide !== null && order !== null && pageStyle !== "plain";

  return <>
    <InlineWorkspace presentation="page" title={guideTitle} description="Study guide"
      onClose={close} closeLabel="Back to Study Guides" actions={topBarActions}
      dismissible={!isGenerating && !isSaving && !isExporting} busy={busy}>
      {isLoading ? (
        <div className="tw:px-page tw:py-10"><LoadingState label="Loading study guide…" /></div>
      ) : !guide || !order ? (
        <div className="tw:mx-auto tw:grid tw:w-full tw:max-w-[42rem] tw:gap-5 tw:px-page tw:py-8 tw:compact:px-4">
          {errorMessage ? (
            <InlineNotice kind="error">
              <p className="tw:mt-0">{errorMessage}</p>
              {loadFailed ? <Button variant="secondary" onClick={load}>Try again</Button> : null}
            </InlineNotice>
          ) : null}
          {loadFailed ? null : (
            <EmptyState icon={<Icon name="study-guide" size={30} />} title="No study guide yet"
              actions={<>
                {isGenerating ? (
                  <Button variant="dangerSecondary" className="tw:border-destructive/30 tw:bg-destructive-soft tw:text-destructive" disabled={isCanceling}
                    aria-label={isCanceling ? "Stopping study guide generation" : "Generating study guide; click to stop"}
                    onClick={() => void stopGeneration()}>
                    <span className="tw:grid tw:size-5 tw:place-items-center tw:text-destructive" aria-hidden="true"><Icon name="stop" size={18} /></span>
                    {isCanceling ? "Stopping study guide…" : "Generating study guide…"}
                  </Button>
                ) : <Button variant="primary" disabled={busy} onClick={() => void generate()}>Generate study guide</Button>}
                <Button variant="secondary" onClick={() => void openSource()}>Open source</Button>
              </>} />
          )}
        </div>
      ) : onCanvas ? (
        <GuideCanvas documentId={documentId} contentKey={contentKey} title={guideTitle} theme={pageStyle} showToast={showToast}
          readTopicIds={readTopicIds} openAtTopicId={currentTopicId} contentsOpen={contentsOpen} onContentsClose={() => setContentsOpen(false)}
          sectionMenu={sectionMenu} onPractice={(topicId) => {
            const section = guide?.sections.find((item) => item.topicId === topicId);
            if (section) void studyTopic(section);
          }}
          onCurrentSection={scrolledTo} leaveRef={canvasLeave} onReadAsText={() => choosePageStyle("plain")} {...links} />
      ) : (
        <div className={layoutMode === "wide"
          ? "tw:mx-auto tw:grid tw:w-full tw:max-w-[80rem] tw:grid-cols-[clamp(10rem,13vw,14rem)_minmax(0,1fr)_clamp(11rem,15vw,15.5rem)] tw:items-start tw:gap-x-[clamp(1.5rem,2.5vw,3rem)] tw:px-page tw:pt-8 tw:pb-[calc(4rem+var(--safe-bottom))]"
          : "tw:w-full tw:px-page tw:pt-7 tw:pb-[calc(4rem+var(--safe-bottom))] tw:compact:px-4 tw:compact:pt-5 tw:compact:pb-6"}>
          {layoutMode === "wide" ? (
            <div className="tw:sticky tw:top-20 tw:max-h-[calc(100dvh-7rem)] tw:min-w-0 tw:overflow-y-auto tw:overscroll-contain tw:rounded-xl tw:bg-foreground/4 tw:p-2">
              <GuideOutline order={order} currentTopicId={currentEntry?.section.topicId ?? null} readTopicIds={readTopicIds} blocks={blocks} onNavigate={navigate} />
            </div>
          ) : null}
          {currentEntry ? (
            <GuideSectionPage key={currentEntry.section.topicId} guide={guide} entry={currentEntry} sectionCount={order.entries.length} layoutMode={layoutMode}
              pageStyle="plain" marks={[]} onWriteOnPage={() => undefined}
              documentId={documentId} documentTitle={documentTitle} source={source} onOpenSource={() => void openSource()}
              projection={topicCards[currentEntry.section.topicId]}
              previous={neighbor(order.entries, currentEntry, -1)} next={neighbor(order.entries, currentEntry, 1)}
              checkProgress={checkProgress[checkKey(currentEntry)] ?? {}}
              onCheckProgressChange={(next) => setCheckProgress((current) => ({ ...current, [checkKey(currentEntry)]: next }))}
              actionsDisabled={actionsDisabled} onNavigate={navigate}
              read={readTopicIds.has(currentEntry.section.topicId)}
              onSetRead={(read) => void setSectionRead(currentEntry.section.topicId, read)}
              onPractice={() => void studyTopic(currentEntry.section)}
              onEditNote={() => beginTopicNote(currentEntry.section)}
              onRemoveNote={(note) => void discardOverlay(note, "Topic note removed.")}
              onEditSection={() => beginSectionOverride(currentEntry.section)}
              onResetSection={(override) => void discardOverlay(override, "Section edit reset.")}
              onDiscardConflict={(conflict) => void discardOverlay(conflict, "Older section edit discarded.")} />
          ) : (
            <GuideOverviewPage guide={guide} guideTitle={guideTitle} order={order} layoutMode={layoutMode} pageStyle="plain"
              documentId={documentId} documentTitle={documentTitle} source={source} onOpenSource={() => void openSource()}
              guideNote={guideNote} actionsDisabled={actionsDisabled} onNavigate={navigate}
              onEditNote={beginGuideNote} onRemoveNote={(note) => void discardOverlay(note, "Guide note removed.")} />
          )}
        </div>
      )}
    </InlineWorkspace>
    {contentsOpen && order && !onCanvas ? (
      <Sheet title="Contents" width="small" onClose={() => setContentsOpen(false)}>
        <GuideOutline order={order} currentTopicId={currentEntry?.section.topicId ?? null} readTopicIds={readTopicIds} blocks={blocks} onNavigate={navigate} showLabel={false} />
      </Sheet>
    ) : null}
    {editing && guide ? (
      <GuideTextDialog key={`${editing.mode}-${editing.topicId ?? "guide"}`} saving={isSaving} initialValue={editing.initialValue}
        width={editing.mode === "override" ? "large" : "medium"}
        {...editingCopy(editing, guide, order?.entries ?? [])}
        onSave={(value) => void saveEditing(value)} onClose={() => setEditing(null)} />
    ) : null}
  </>;
}

function neighbor(entries: GuideSectionEntry[], entry: GuideSectionEntry, step: -1 | 1): GuidePageLink | null {
  const index = entries.indexOf(entry) + step;
  if (index < 0) return { topicId: null, number: null, title: "Overview" };
  const next = entries[index];
  return next ? { topicId: next.section.topicId, number: next.number, title: next.title } : null;
}

function checkKey(entry: GuideSectionEntry): string {
  return `${entry.section.topicId}:${entry.section.contentHash}`;
}

function editingCopy(editing: EditingState, guide: DocumentStudyGuide, entries: GuideSectionEntry[]): { title: string; description: string; label: string } {
  const existing = findEditingOverlay(guide.overlays, editing.topicId, editing.mode, guide.id) !== null;
  const sectionTitle = entries.find((entry) => entry.section.topicId === editing.topicId)?.title ?? "";
  if (editing.mode === "override") return {
    title: "Edit or rewrite this section",
    description: "Your version replaces the generated text in the reader. The generated section is kept, so you can reset to it at any time. Clear the text and save to reset it now.",
    label: "Section text",
  };
  if (editing.topicId === null) return {
    title: existing ? "Edit guide note" : "Add a guide note",
    description: "A note about the whole guide. It stays with this document when the guide is regenerated. Clear the text and save to remove it.",
    label: "Note",
  };
  return {
    title: existing ? "Edit note" : "Add a note",
    description: `A note on “${sectionTitle}”. It stays with this topic when the guide is regenerated. Clear the text and save to remove it.`,
    label: "Note",
  };
}

import { useEffect, useId, useRef, useState } from "react";
import { useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import { getSemanticSourceCitation, sameStudyScope, studyScopeKey, type SourceCitation } from "../../ipc/knowledge";
import type { TutorTurn } from "../../ipc/tutor";
import { copyText } from "../../platform/clipboard";
import { Icon } from "../../ui/icons/Icon";
import { IconButton } from "../../ui/icons/IconButton";
import { cn } from "../../ui/internal/cn";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { Dialog, Sheet } from "../../ui/overlays/Dialog";
import { OverflowMenu, type OverflowMenuItem } from "../../ui/overlays/OverflowMenu";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { Textarea, Disclosure } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { TutorChatSettings } from "./TutorChatSettings";
import { RenameConversationDialog, TutorHistory } from "./TutorHistory";
import { useTutor } from "./TutorContext";
import { TutorFigure } from "./TutorFigure";
import { TutorMarkdown } from "./TutorMarkdown";
import { sourcesLabel, TutorSources } from "./TutorSources";
import { useArtwork } from "../../ui/artwork/Artwork";
import { useLiveTurn } from "./useLiveTurn";
import { useSpeechInput } from "./useSpeechInput";
import { TutorStarters, useTutorStarters, type TutorStarter } from "./TutorStarters";
import { styles, withoutFigures } from "./presentation";

function SourceDetails({ source }: { source: SourceCitation }) {
  return (
    <>
      <p className={styles.message}>{source.excerpt}</p>
      {source.locator ? <small>Source units: {source.locator.unitNumbers.join(", ")}</small> : null}
      <small>{source.origin.replaceAll("_", " ")}</small>
    </>
  );
}

/** Copy reports its own result in place, so a chat needs no toast. */
function CopyAnswer({ text }: { text: string }) {
  const [result, setResult] = useState<"copied" | "failed" | null>(null);
  useEffect(() => {
    if (!result) return;
    const timer = window.setTimeout(() => setResult(null), 2000);
    return () => window.clearTimeout(timer);
  }, [result]);
  return (
    <Button size="small" variant="ghost" onClick={() => {
      copyText(text).then(() => setResult("copied"), () => setResult("failed"));
    }}>
      <Icon name="copy" size={16} />
      <span aria-live="polite">{result === "copied" ? "Copied" : result === "failed" ? "Couldn't copy" : "Copy"}</span>
    </Button>
  );
}

export function TutorWorkspace({ showHistory = false }: { showHistory?: boolean }) {
  const tutor = useTutor();
  const id = useId();
  const confirm = useConfirm();
  const { layoutMode } = useLayoutMode();
  const [historyOpen, setHistoryOpen] = useState(false);
  const [sourcesOpen, setSourcesOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [source, setSource] = useState<SourceCitation | null>(null);
  const [sourceError, setSourceError] = useState<string | null>(null);
  const [sourceBusy, setSourceBusy] = useState(false);
  const sourceLane = useRequestLane();
  const transcript = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  const followTail = useRef(true);
  const [awayFromTail, setAwayFromTail] = useState(false);
  const starters = useTutorStarters();
  const artwork = useArtwork("tutor.welcome");
  const welcomeArt = artwork.enabled ? artwork.definition.image : undefined;
  const hintOnly = tutor.target?.mode === "hint";
  const locked = tutor.busy || tutor.loading || tutor.pending !== null || tutor.preparation.locked;
  const question = [...(tutor.detail?.turns ?? [])].reverse().find((turn) => turn.pendingQuestion);
  const turns = (tutor.detail?.turns ?? []).filter((turn) => !hintOnly || turn.mode === "hint");
  const lastTurnId = tutor.detail?.turns.at(-1)?.id;
  const live = useLiveTurn(tutor.liveOperationId);
  const withDraft = (text: string) => (tutor.draft.trim() ? `${tutor.draft.trimEnd()} ${text}` : text);
  const speech = useSpeechInput({
    onText: (text) => {
      tutor.setDraft(withDraft(text));
      input.current?.focus();
    },
    // A question that ended on a pause is sent as heard; if sending cannot
    // start now, it waits in the draft.
    onPause: (text) => {
      const spoken = withDraft(text);
      tutor.setDraft(spoken);
      void tutor.send(hintOnly ? "hint" : question ? "answer" : "explain", spoken);
    },
  });
  const liveAnswer = (
    <div className={styles.assistantMessage} data-testid="tutor-live-answer">
      {live?.draft && !hintOnly ? <div aria-busy="true"><TutorMarkdown text={live.draft} /></div> : null}
      <p role="status" className={styles.muted}>{live?.status ?? (hintOnly ? "Preparing a hint…" : "Thinking…")}</p>
    </div>
  );
  const chosen = tutor.target?.title ?? sourcesLabel(tutor.scope, tutor.choices);
  const inlineHistory = showHistory && layoutMode !== "compact" && !hintOnly;
  // Only the wide, full-page chat has room to keep a source open beside it.
  const sourceBeside = inlineHistory && layoutMode === "wide";
  const canQuiz = !locked && !tutor.needsRefresh && (tutor.scope.kind !== "general" || Boolean(tutor.target) || Boolean(tutor.draft.trim()));
  // A new full-page chat puts its composer in the middle, under the raccoon.
  const welcome = !hintOnly && showHistory && !turns.length && !tutor.detail?.hasEarlierTurns && !tutor.busy && !tutor.pending;
  // Phones keep Enter for new lines; the send button sends.
  const enterSends = layoutMode !== "compact";

  useEffect(() => {
    sourceLane.invalidate();
    setSource(null);
    setSourceError(null);
    setSourceBusy(false);
    followTail.current = true;
    setAwayFromTail(false);
  }, [sourceLane, tutor.detail?.conversation.id, hintOnly]);

  useEffect(() => {
    if (followTail.current && transcript.current) transcript.current.scrollTop = transcript.current.scrollHeight;
  }, [tutor.detail, tutor.busy, live?.draft]);

  // The composer grows with its text up to its CSS maximum, then scrolls.
  useEffect(() => {
    const element = input.current;
    if (!element) return;
    element.style.height = "auto";
    element.style.height = `${element.scrollHeight}px`;
  }, [tutor.draft, welcome]);

  function jumpToLatest(): void {
    const element = transcript.current;
    if (!element) return;
    followTail.current = true;
    setAwayFromTail(false);
    element.scrollTop = element.scrollHeight;
  }

  function startFrom(starter: TutorStarter): void {
    void tutor.start(starter.scope, starter.mode, starter.text, starter.chatTitle);
  }

  async function showSource(citation: SourceCitation): Promise<void> {
    if (hintOnly) return;
    const token = sourceLane.begin();
    setSourceBusy(true);
    setSourceError(null);
    try {
      const value = await getSemanticSourceCitation({ sourceId: citation.id, contentHash: citation.contentHash });
      if (sourceLane.isCurrent(token)) setSource(value);
    } catch (failure: unknown) {
      if (sourceLane.isCurrent(token)) setSourceError(getErrorMessage(failure));
    } finally {
      if (sourceLane.isCurrent(token)) setSourceBusy(false);
    }
  }

  async function remove(): Promise<void> {
    if (await confirm({
      title: "Delete this conversation?",
      description: "Only this chat is deleted. Documents, cards, study guides and review schedules stay unchanged.",
      confirmLabel: "Delete conversation",
      destructive: true,
    })) {
      await tutor.removeConversation();
      setSettingsOpen(false);
    }
  }

  function submit(): void {
    void tutor.send(hintOnly ? "hint" : question ? "answer" : "explain");
  }

  function openSources(): void {
    setSourcesOpen(true);
    void tutor.initialize();
  }

  const chatActions: OverflowMenuItem[] = [
    ...(tutor.detail ? [{ id: "rename", label: "Rename…", onSelect: () => setRenaming(true) }] : []),
    { id: "settings", label: "Chat settings", onSelect: () => setSettingsOpen(true) },
    ...(tutor.detail ? [{ id: "delete", label: "Delete", destructive: true, onSelect: () => { void remove(); } }] : []),
  ];

  // Sources are optional: with none chosen the Tutor answers from what it knows.
  const scopePicker = (
    <Button
      size="small"
      variant="secondary"
      className={styles.scope}
      aria-label={chosen ? `Sources: ${chosen}` : "Add sources"}
      disabled={locked}
      onClick={openSources}
    >
      <Icon name={chosen ? "book" : "plus"} size={16} />
      <span>{chosen ?? "Add sources"}</span>
      {chosen ? <Icon name="chevron-down" size={16} /> : null}
    </Button>
  );

  function citations(turn: TutorTurn) {
    if (hintOnly || turn.mode === "quiz" || !turn.citations.length) return null;
    return (
      <div className={styles.citations}>
        {turn.citations.map((citation, index) => (
          <Button
            key={citation.id}
            size="small"
            variant="secondary"
            className={styles.citation}
            aria-label={`Source ${index + 1}: ${citation.title}`}
            title={citation.title}
            disabled={sourceBusy || !turn.evidenceCurrent}
            onClick={() => { void showSource(citation); }}
          >
            {index + 1}
          </Button>
        ))}
      </div>
    );
  }

  const messageLabel = hintOnly ? "Hint request" : question ? "Your answer" : "Message";
  const composerInput = (className: string, rows: number) => (
    <>
      <label className={styles.composerLabel} htmlFor={`${id}-message`}>{messageLabel}</label>
      <Textarea
        ref={input}
        className={className}
        id={`${id}-message`}
        rows={rows}
        maxLength={8000}
        value={tutor.draft}
        placeholder={question ? "Your answer" : turns.length ? "Ask a follow-up" : "Ask anything"}
        autoFocus={welcome && layoutMode !== "compact"}
        disabled={locked || tutor.needsRefresh}
        onChange={(event) => tutor.setDraft(event.target.value)}
        onKeyDown={(event) => {
          if (event.key !== "Enter" || event.nativeEvent.isComposing) return;
          if (event.ctrlKey || event.metaKey || (enterSends && !hintOnly && !event.shiftKey && !event.altKey)) {
            event.preventDefault();
            submit();
          }
        }}
      />
    </>
  );
  const submitLabel = hintOnly ? "Request hint" : question ? "Submit answer" : "Send";
  const recording = speech.phase === "recording";
  const sendDisabled = locked || tutor.needsRefresh || !tutor.draft.trim() || speech.phase !== "idle";
  const speechStatus = recording
    ? "Recording…"
    : speech.phase === "transcribing"
      ? "Turning speech into text…"
      : speech.phase === "downloading"
        ? `Downloading speech input… ${Math.round(speech.downloaded * 100)}%`
        : null;
  const microphone = speech.available ? (
    <>
      {recording ? (
        <>
          <span className={styles.recordingTime} aria-hidden="true">
            {Math.floor(speech.seconds / 60)}:{String(speech.seconds % 60).padStart(2, "0")}
          </span>
          <span className={styles.level} aria-hidden="true">
            <span style={{ width: `${Math.round(speech.level * 100)}%` }} />
          </span>
          <IconButton icon="close" label="Discard recording" onClick={speech.cancel} />
        </>
      ) : null}
      <IconButton
        icon="mic"
        label={recording ? "Stop and use recording" : "Speak"}
        aria-pressed={recording}
        className={recording ? styles.micRecording : undefined}
        loading={speech.phase === "transcribing" || speech.phase === "downloading"}
        disabled={!recording && (speech.phase !== "idle" || locked || tutor.needsRefresh)}
        onClick={speech.toggle}
      />
    </>
  ) : null;

  const composerForm = (
    <form
      className={styles.composer}
      onSubmit={(event) => { event.preventDefault(); submit(); }}
      onKeyDown={(event) => {
        if (event.key === "Escape" && recording) {
          event.preventDefault();
          speech.cancel();
        }
      }}
    >
      {composerInput(styles.composerInput, 1)}
      {microphone}
      {question ? (
        <Button size="small" variant="ghost" className="tw:mb-1" disabled={sendDisabled} onClick={() => { void tutor.send("explain"); }}>Ask instead</Button>
      ) : null}
      {tutor.busy || tutor.pending ? (
        <IconButton icon="stop" label="Stop response" className={styles.stop} onClick={() => { void tutor.cancel(); }} />
      ) : (
        <IconButton icon="arrow-up" type="submit" label={submitLabel} className={styles.send} disabled={sendDisabled} />
      )}
    </form>
  );
  const composer = (
    <>
      {composerForm}
      {speech.available ? (
        <p className={speech.error ? styles.speechError : styles.speechStatus} role="status">
          {speech.error ?? speechStatus}
        </p>
      ) : null}
    </>
  );

  const transcriptView = (
    <div
      ref={transcript}
      className={hintOnly ? styles.hintTranscript : styles.transcript}
      role="log"
      aria-label="Saved conversation"
      aria-relevant="additions"
      onScroll={() => {
        const element = transcript.current;
        if (!element) return;
        followTail.current = element.scrollHeight - element.scrollTop - element.clientHeight < 100;
        setAwayFromTail(!followTail.current);
      }}
    >
      {welcome ? (
        <div className={styles.welcome}>
          {welcomeArt ? (
            <img
              src={welcomeArt.src}
              srcSet={welcomeArt.srcSet}
              alt=""
              draggable={false}
              data-artwork-slot="tutor.welcome"
              className={styles.welcomeArt}
            />
          ) : null}
          <h3>What are we studying today?</h3>
          <div className={styles.welcomeComposer}>{composer}</div>
          <div className={styles.toolbar}>
            {scopePicker}
            <Button size="small" variant="secondary" className={styles.scope} disabled={!canQuiz} onClick={() => { void tutor.send("quiz"); }}>
              <Icon name="help" size={16} /> Quiz me
            </Button>
          </div>
          <TutorStarters starters={starters} disabled={locked || tutor.needsRefresh} onStart={startFrom} />
        </div>
      ) : !turns.length && !tutor.busy ? (
        <div className={styles.empty}>
          <h3>{hintOnly ? "Work toward the answer" : "What would you like to understand?"}</h3>
          {!hintOnly ? (
            <div className={styles.toolbar}>
              {scopePicker}
              <Button size="small" variant="secondary" className={styles.scope} disabled={!canQuiz} onClick={() => { void tutor.send("quiz"); }}>
                <Icon name="help" size={16} /> Quiz me
              </Button>
            </div>
          ) : null}
        </div>
      ) : null}
      {tutor.detail?.hasEarlierTurns && !hintOnly ? (
        <Button className="tw:self-center" disabled={locked} onClick={() => { followTail.current = false; void tutor.loadEarlier(); }}>Load earlier messages</Button>
      ) : null}
      {turns.map((turn) => {
        const answered = turn.state === "complete" && Boolean(turn.assistantText) && turn.assistantText !== turn.errorMessage;
        return (
          <article className={styles.turn} key={turn.id}>
            {turn.userText ? (
              <section className={styles.userMessage} aria-label="Your message">
                <div className={styles.message}>{turn.userText}</div>
              </section>
            ) : null}
            <section className={styles.assistantMessage} aria-label="Tutor response">
              {turn.assistantText && turn.assistantText !== turn.errorMessage
                ? turn.mode === "hint"
                  ? <div className={styles.message}>{turn.assistantText}</div>
                  : (
                    // Floated figures stay inside their answer.
                    <div className="tw:flow-root">
                      <TutorMarkdown
                        text={turn.assistantText}
                        figure={turn.state === "complete"
                          ? (reference, title) => <TutorFigure turnId={turn.id} reference={reference} title={title} />
                          : undefined}
                      />
                    </div>
                  )
                : null}
              {turn.state === "running" ? liveAnswer : null}
              {turn.errorMessage ? <InlineNotice kind="error">{turn.errorMessage}</InlineNotice> : null}
              {!turn.evidenceCurrent && turn.state === "complete" ? (
                <InlineNotice kind="warning">The source has changed or been removed. This answer is historical, not current evidence.</InlineNotice>
              ) : null}
              {turn.feedback ? (
                <Disclosure title="Quiz feedback">
                  {turn.feedback.criteriaFeedback.map((text, index) => <TutorMarkdown key={index} text={text} />)}
                </Disclosure>
              ) : null}
              {citations(turn)}
              {!hintOnly && answered ? (
                <div className={styles.answerActions}>
                  {turn.id === lastTurnId && !question ? (
                    <Button size="small" variant="ghost" disabled={locked || tutor.needsRefresh} onClick={() => { void tutor.send("quiz", ""); }}>
                      <Icon name="help" size={16} /> Quiz me on this
                    </Button>
                  ) : null}
                  <CopyAnswer text={withoutFigures(turn.assistantText)} />
                </div>
              ) : null}
              {turn.id === lastTurnId
                && (turn.mode !== "hint" || hintOnly)
                && ["failed", "canceled", "interrupted"].includes(turn.state) ? (
                  <Button size="small" className="tw:justify-self-start" disabled={locked || tutor.needsRefresh} onClick={() => { void tutor.retryTurn(turn); }}>
                    Try again
                  </Button>
                ) : null}
            </section>
          </article>
        );
      })}
      {tutor.busy && !turns.some((turn) => turn.state === "running") ? (
        <div className="tw:mx-auto tw:w-full tw:max-w-3xl">{liveAnswer}</div>
      ) : null}
    </div>
  );

  const notices = (
    <div className={hintOnly ? "tw:grid tw:gap-2" : styles.notices}>
      {!tutor.profiles.length && !tutor.loading ? (
        <InlineNotice>Choose a Tutor model in Settings → AI to send messages.</InlineNotice>
      ) : null}
      {sourceError ? <InlineNotice kind="error">{sourceError}</InlineNotice> : null}
      {tutor.error ? <InlineNotice kind="error">{tutor.error}</InlineNotice> : null}
      {tutor.needsRefresh ? <Button disabled={tutor.busy} onClick={() => { void tutor.refresh(); }}>Refresh saved conversation</Button> : null}
      {tutor.pending ? (
        <InlineNotice>
          <p>Checking this response will not submit a duplicate message.</p>
          <Button disabled={tutor.busy} onClick={() => { void tutor.retryPending(); }}>Check response</Button>
        </InlineNotice>
      ) : null}
    </div>
  );

  if (hintOnly) {
    return (
      <section className={styles.panelWorkspace} aria-label="Flashcard hints">
        <div className={styles.hintChat}>
          {notices}
          {transcriptView}
          <form className={styles.hintComposer} onSubmit={(event) => { event.preventDefault(); submit(); }}>
            {composerInput(styles.hintComposerInput, 2)}
            <div className={styles.toolbar}>
              <Button type="submit" variant="primary" disabled={sendDisabled}>{submitLabel}</Button>
              {tutor.busy || tutor.pending ? (
                <Button variant="dangerSecondary" onClick={() => { void tutor.cancel(); }}><Icon name="stop" /> Stop response</Button>
              ) : null}
            </div>
          </form>
        </div>
      </section>
    );
  }

  const header = (
    <header className={styles.chatHeader}>
      {!inlineHistory ? (
        <IconButton icon="menu" label="Conversations" disabled={locked} onClick={() => setHistoryOpen(true)} />
      ) : null}
      <h2 className={styles.heading}>{tutor.detail?.conversation.title ?? "New chat"}</h2>
      {inlineHistory ? (turns.length ? scopePicker : null) : (
        <IconButton icon="plus" label="New chat" disabled={locked || tutor.organizing} onClick={() => tutor.newConversation()} />
      )}
      <OverflowMenu label="Chat actions" disabled={locked} items={chatActions} />
      {!inlineHistory && turns.length ? <div className={styles.scopeRow}>{scopePicker}</div> : null}
    </header>
  );

  return (
    <section
      className={inlineHistory ? cn(styles.withHistory, sourceBeside && source ? styles.withSource : null) : showHistory ? styles.workspace : styles.panelWorkspace}
      aria-label="Tutor conversation"
    >
      {inlineHistory ? <aside className={styles.historyPane}><TutorHistory tutor={tutor} /></aside> : null}
      <div className={styles.chat}>
        {header}
        <div className={styles.transcriptFrame}>
          {transcriptView}
          {awayFromTail && turns.length ? (
            <Button size="small" className={styles.jump} onClick={jumpToLatest}>
              <Icon name="arrow-down" size={16} /> Jump to latest
            </Button>
          ) : null}
        </div>
        {notices}
        {!welcome ? <div className={styles.composerWrap}>{composer}</div> : null}
      </div>
      {sourceBeside && source ? (
        <aside className={styles.sourcePanel} aria-label="Source">
          <div className={styles.sourcePanelHeader}>
            <h3>Source</h3>
            <IconButton icon="close" label="Close source" onClick={() => setSource(null)} />
          </div>
          <div className={styles.sourceCard}>
            <strong>{source.title}</strong>
            <SourceDetails source={source} />
          </div>
        </aside>
      ) : null}
      {historyOpen ? (
        <Sheet title="Conversations" onClose={() => setHistoryOpen(false)}>
          <TutorHistory tutor={tutor} onSelected={() => setHistoryOpen(false)} />
        </Sheet>
      ) : null}
      {sourcesOpen ? (
        <TutorSources
          key={studyScopeKey(tutor.scope)}
          scope={tutor.scope}
          choices={tutor.choices}
          onClose={() => setSourcesOpen(false)}
          onApply={(scope) => { if (!sameStudyScope(scope, tutor.scope)) tutor.changeSources(scope); }}
        />
      ) : null}
      {settingsOpen ? <TutorChatSettings tutor={tutor} onClose={() => setSettingsOpen(false)} onDelete={remove} /> : null}
      {renaming && tutor.detail ? (
        <RenameConversationDialog tutor={tutor} conversation={tutor.detail.conversation} onClose={() => setRenaming(false)} />
      ) : null}
      {source && !sourceBeside ? (
        <Dialog title={source.title} onClose={() => setSource(null)} width="large">
          <div className="tw:grid tw:gap-2 tw:[&>small]:text-muted-foreground">
            <SourceDetails source={source} />
          </div>
        </Dialog>
      ) : null}
    </section>
  );
}

export function TutorScreen() {
  const tutor = useTutor();
  const initialize = tutor.initialize;
  useEffect(() => { void initialize(); }, [initialize]);
  return (
    <PageScaffold width="full" inset={false} className={styles.screen} data-testid="tutor-screen">
      <h1 className="tw:sr-only">Tutor</h1>
      {tutor.loading ? <p role="status" className="tw:sr-only">Loading saved chats…</p> : null}
      <TutorWorkspace showHistory />
    </PageScaffold>
  );
}

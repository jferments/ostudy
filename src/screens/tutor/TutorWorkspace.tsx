import { useEffect, useId, useRef, useState } from "react";
import { useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import { getSemanticSourceCitation, sameStudyScope, scopeMembers, studyScopeKey, type SourceCitation } from "../../ipc/knowledge";
import { Icon } from "../../ui/icons/Icon";
import { IconButton } from "../../ui/icons/IconButton";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { Dialog, Sheet } from "../../ui/overlays/Dialog";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { Textarea, Disclosure } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { EmptyState } from "../../ui/primitives/EmptyState";
import { PageScaffold, PageHeader } from "../../ui/patterns/WorkspacePatterns";
import { TutorChatSettings } from "./TutorChatSettings";
import { useTutor } from "./TutorContext";
import { TutorMarkdown } from "./TutorMarkdown";
import { TutorSources } from "./TutorSources";
import { styles } from "./presentation";

export function TutorWorkspace({ showHistory = false }: { showHistory?: boolean }) {
  const tutor = useTutor();
  const id = useId();
  const confirm = useConfirm();
  const { layoutMode } = useLayoutMode();
  const [historyOpen, setHistoryOpen] = useState(false);
  const [sourcesOpen, setSourcesOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [source, setSource] = useState<SourceCitation | null>(null);
  const [sourceError, setSourceError] = useState<string | null>(null);
  const [sourceBusy, setSourceBusy] = useState(false);
  const sourceLane = useRequestLane();
  const transcript = useRef<HTMLDivElement>(null);
  const followTail = useRef(true);
  const hintOnly = tutor.target?.mode === "hint";
  const locked = tutor.busy || tutor.loading || tutor.pending !== null || tutor.preparation.locked;
  const question = [...(tutor.detail?.turns ?? [])].reverse().find((turn) => turn.pendingQuestion);
  const turns = (tutor.detail?.turns ?? []).filter((turn) => !hintOnly || turn.mode === "hint");
  const members = scopeMembers(tutor.scope);
  const sourceLabel = tutor.scope.kind === "library"
    ? "Library"
    : members.length > 1
      ? `${members.length} sources`
      : members.length === 1
        ? tutor.choices.find((choice) => sameStudyScope(choice.scope, members[0]))?.title ?? "Selected source"
        : null;
  const headingContext = tutor.target?.title ?? sourceLabel;
  const inlineHistory = showHistory && layoutMode !== "compact" && !hintOnly;

  useEffect(() => {
    sourceLane.invalidate();
    setSource(null);
    setSourceError(null);
    setSourceBusy(false);
    followTail.current = true;
  }, [sourceLane, tutor.detail?.conversation.id, hintOnly]);

  useEffect(() => {
    if (followTail.current && transcript.current) transcript.current.scrollTop = transcript.current.scrollHeight;
  }, [tutor.detail, tutor.busy]);

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

  function history() {
    const conversations = tutor.detail && !tutor.conversations.some((saved) => saved.id === tutor.detail?.conversation.id)
      ? [tutor.detail.conversation, ...tutor.conversations]
      : tutor.conversations;
    return (
      <nav className={styles.history} aria-label="Conversations">
        <Button disabled={locked} onClick={() => { tutor.newConversation(); setHistoryOpen(false); }}>
          <Icon name="plus" /> New chat
        </Button>
        {!conversations.length ? <p className={styles.muted}>Your conversations appear here.</p> : null}
        {conversations.map((saved) => (
          <Button
            variant="ghost"
            key={saved.id}
            className={styles.historyItem}
            aria-current={tutor.detail?.conversation.id === saved.id ? "page" : undefined}
            disabled={locked}
            onClick={() => { void tutor.selectConversation(saved.id); setHistoryOpen(false); }}
          >
            {saved.title}
          </Button>
        ))}
      </nav>
    );
  }

  return (
    <section className={inlineHistory ? styles.withHistory : styles.workspace} aria-label={hintOnly ? "Flashcard hints" : "Tutor conversation"}>
      {inlineHistory ? <aside className={styles.historyPane}>{history()}</aside> : null}
      <div className={styles.chat}>
        {!hintOnly ? (
          <header className={styles.chatHeader}>
            <div className={styles.heading}>
              <h2>{tutor.detail?.conversation.title ?? "New chat"}</h2>
              {headingContext ? <span className={styles.muted}>{headingContext}</span> : null}
            </div>
            <div className={styles.toolbar}>
              {!inlineHistory ? (
                <Button size="small" disabled={locked} onClick={() => setHistoryOpen(true)}>
                  <Icon name="menu" /> Conversations
                </Button>
              ) : null}
              <Button size="small" disabled={locked} onClick={() => { setSourcesOpen(true); void tutor.initialize(); }}>
                <Icon name="document" /> Sources
              </Button>
              <IconButton
                icon="settings"
                iconSize={20}
                label="Chat settings"
                tooltip="Chat settings"
                disabled={locked}
                onClick={() => setSettingsOpen(true)}
              />
            </div>
          </header>
        ) : (
          <p className={styles.muted}>Hints keep the answer and source previews hidden. Close this panel to return to the same card.</p>
        )}
        {!tutor.profiles.length && !tutor.loading ? (
          <InlineNotice>Select a Tutor model in Settings → AI/LLM to send messages. Saved chats remain readable offline.</InlineNotice>
        ) : null}
        <div
          ref={transcript}
          className={styles.transcript}
          role="log"
          aria-label="Saved conversation"
          aria-relevant="additions"
          onScroll={() => {
            const element = transcript.current;
            if (element) followTail.current = element.scrollHeight - element.scrollTop - element.clientHeight < 100;
          }}
        >
          {!turns.length ? (
            <EmptyState
              className={styles.empty}
              icon={<Icon name="brain" size={32} />}
              title={hintOnly ? "Work toward the answer" : "What would you like to understand?"}
              description={hintOnly
                ? "Ask for an explanation, compare ideas, or test your knowledge."
                : "Ask for an explanation, compare ideas, or test your knowledge. Choose optional sources above. Your saved material is ready to use; no separate understanding step."}
            />
          ) : null}
          {tutor.detail?.hasEarlierTurns && !hintOnly ? (
            <Button disabled={locked} onClick={() => { followTail.current = false; void tutor.loadEarlier(); }}>Load earlier messages</Button>
          ) : null}
          {turns.map((turn) => (
            <article className={styles.turn} key={turn.id}>
              {turn.userText ? (
                <section className={styles.userMessage} aria-label="Your message">
                  <strong>You</strong><div className={styles.message}>{turn.userText}</div>
                </section>
              ) : null}
              <section className={styles.assistantMessage} aria-label="Tutor response">
                <div className={styles.toolbar}><strong>Tutor</strong><span className={styles.muted}>{turn.model}</span></div>
                {turn.assistantText && turn.assistantText !== turn.errorMessage
                  ? turn.mode === "hint"
                    ? <div className={styles.message}>{turn.assistantText}</div>
                    : <TutorMarkdown text={turn.assistantText} />
                  : null}
                {turn.state === "running" ? <p role="status">Working on your response…</p> : null}
                {turn.errorMessage ? <InlineNotice kind="error">{turn.errorMessage}</InlineNotice> : null}
                {!turn.evidenceCurrent && turn.state === "complete" ? (
                  <InlineNotice kind="warning">The source has changed or been removed. This answer is historical, not current evidence.</InlineNotice>
                ) : null}
                {turn.feedback ? (
                  <Disclosure title="Quiz feedback">
                    {turn.feedback.criteriaFeedback.map((text, index) => <TutorMarkdown key={index} text={text} />)}
                  </Disclosure>
                ) : null}
                {!hintOnly && turn.mode !== "quiz" ? (
                  <div className={styles.sources}>
                    {turn.citations.map((citation, index) => (
                      <Button
                        key={citation.id}
                        size="small"
                        variant="text"
                        disabled={sourceBusy || !turn.evidenceCurrent}
                        onClick={() => { void showSource(citation); }}
                      >
                        Source {index + 1}: {citation.title}
                      </Button>
                    ))}
                  </div>
                ) : null}
                {turn.id === tutor.detail?.turns.at(-1)?.id
                  && (turn.mode !== "hint" || hintOnly)
                  && ["failed", "canceled", "interrupted"].includes(turn.state) ? (
                    <Button size="small" disabled={locked || tutor.needsRefresh} onClick={() => { void tutor.retryTurn(turn); }}>
                      Try again
                    </Button>
                  ) : null}
              </section>
            </article>
          ))}
          {tutor.busy && !turns.some((turn) => turn.state === "running") ? (
            <p role="status">{hintOnly ? "Preparing a hint…" : "Searching saved material and composing your response…"}</p>
          ) : null}
        </div>
        {sourceError ? <InlineNotice kind="error">{sourceError}</InlineNotice> : null}
        {tutor.error ? <InlineNotice kind="error">{tutor.error}</InlineNotice> : null}
        {tutor.needsRefresh ? <Button disabled={tutor.busy} onClick={() => { void tutor.refresh(); }}>Refresh saved conversation</Button> : null}
        {tutor.pending ? (
          <InlineNotice>
            <p>Checking this response will not submit a duplicate message.</p>
            <Button disabled={tutor.busy} onClick={() => { void tutor.retryPending(); }}>Check response</Button>
          </InlineNotice>
        ) : null}
        <form className={styles.composer} onSubmit={(event) => { event.preventDefault(); submit(); }}>
          <label className={styles.composerLabel} htmlFor={`${id}-message`}>{hintOnly ? "Hint request" : question ? "Your answer" : "Message"}</label>
          <Textarea
            className={styles.composerInput}
            id={`${id}-message`}
            rows={2}
            maxLength={8000}
            value={tutor.draft}
            placeholder={question ? "Type your answer…" : "Ask a question…"}
            disabled={locked || tutor.needsRefresh}
            onChange={(event) => tutor.setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && (event.ctrlKey || event.metaKey) && !event.nativeEvent.isComposing) {
                event.preventDefault();
                submit();
              }
            }}
          />
          <div className={styles.toolbar}>
            <Button type="submit" variant="primary" disabled={locked || tutor.needsRefresh || !tutor.draft.trim()}>
              {hintOnly ? "Request hint" : question ? "Submit answer" : "Send"}
            </Button>
            {tutor.busy || tutor.pending ? (
              <Button variant="dangerSecondary" onClick={() => { void tutor.cancel(); }}><Icon name="stop" /> Stop response</Button>
            ) : null}
            {!hintOnly && question ? (
              <Button disabled={locked || tutor.needsRefresh || !tutor.draft.trim()} onClick={() => { void tutor.send("explain"); }}>Ask instead</Button>
            ) : null}
          </div>
          <p className={styles.disclosure}>Sending uses your selected AI and embedding services with relevant material. Charges may apply. AI can make mistakes.</p>
        </form>
      </div>
      {historyOpen && !hintOnly ? <Sheet title="Conversations" onClose={() => setHistoryOpen(false)}>{history()}</Sheet> : null}
      {sourcesOpen && !hintOnly ? (
        <TutorSources
          key={studyScopeKey(tutor.scope)}
          scope={tutor.scope}
          choices={tutor.choices}
          onClose={() => setSourcesOpen(false)}
          onApply={(scope) => { if (!sameStudyScope(scope, tutor.scope)) tutor.changeSources(scope); }}
        />
      ) : null}
      {settingsOpen && !hintOnly ? <TutorChatSettings tutor={tutor} onClose={() => setSettingsOpen(false)} onDelete={remove} /> : null}
      {source && !hintOnly ? (
        <Dialog title={source.title} onClose={() => setSource(null)} width="large">
          <p className={styles.message}>{source.excerpt}</p>
          {source.locator ? <p>Source units: {source.locator.unitNumbers.join(", ")}</p> : null}
          <p className={styles.muted}>Origin: {source.origin.replaceAll("_", " ")}. A citation links the source; it does not independently verify every claim.</p>
        </Dialog>
      ) : null}
    </section>
  );
}

export function TutorScreen() {
  const tutor = useTutor();
  useEffect(() => { void tutor.initialize(); }, [tutor.initialize]);
  return (
    <PageScaffold width="full" className={styles.screen} data-testid="tutor-screen">
      <PageHeader title="Tutor" icon="brain" description="Explore ideas with your study material, or start a general conversation." />
      {tutor.loading ? <p role="status">Loading saved chats and settings…</p> : null}
      <TutorWorkspace showHistory />
    </PageScaffold>
  );
}

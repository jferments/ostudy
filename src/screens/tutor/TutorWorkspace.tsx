import { useEffect, useId, useState } from "react";

import { getErrorMessage } from "../../ipc/common";
import { getSemanticSourceCitation, studyScopeKey, type SourceCitation, type StudyScope } from "../../ipc/knowledge";
import { Dialog } from "../../ui/overlays/Dialog";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { useTutor } from "./TutorContext";
import styles from "./TutorWorkspace.module.css";

export function TutorWorkspace() {
  const tutor = useTutor();
  const id = useId();
  const confirm = useConfirm();
  const [source, setSource] = useState<SourceCitation | null>(null);
  const [sourceError, setSourceError] = useState<string | null>(null);
  const [sourceBusy, setSourceBusy] = useState(false);
  const hintOnly = tutor.target?.mode === "hint";
  const locked = tutor.busy || tutor.loading || tutor.pending !== null;
  const profiles = tutor.profiles.filter((profile) => profile.enabled && profile.configurationStatus === "ready");
  const question = [...(tutor.detail?.turns ?? [])].reverse().find((turn) => turn.pendingQuestion);
  const scopes = [
    { scope: { kind: "general", id: null } as StudyScope, title: "General discussion" },
    ...tutor.choices,
  ];
  const scopeKey = studyScopeKey(tutor.scope);
  const turns = (tutor.detail?.turns ?? []).filter((turn) => !hintOnly || turn.mode === "hint");

  useEffect(() => { setSource(null); setSourceError(null); }, [tutor.detail?.conversation.id, hintOnly]);

  async function showSource(citation: SourceCitation): Promise<void> {
    if (hintOnly) return;
    setSourceBusy(true);
    setSourceError(null);
    try { setSource(await getSemanticSourceCitation({ sourceId: citation.id, contentHash: citation.contentHash })); }
    catch (failure: unknown) { setSourceError(getErrorMessage(failure)); }
    finally { setSourceBusy(false); }
  }

  async function remove(): Promise<void> {
    if (await confirm({ title: "Delete this conversation?", description: "Messages and private quiz criteria will be removed. Cards, source documents and review schedules will not change.", confirmLabel: "Delete conversation", destructive: true })) {
      await tutor.removeConversation();
    }
  }

  function submit(): void {
    void tutor.send(hintOnly ? "hint" : question ? "answer" : "explain");
  }

  return (
    <section className={styles.workspace} aria-label={hintOnly ? "Flashcard hints" : "Tutor conversation"}>
      {!hintOnly ? (
        <div className={styles.toolbar}>
          <label className={styles.field} htmlFor={`${id}-conversation`}>Conversation
            <select id={`${id}-conversation`} value={tutor.detail?.conversation.id ?? ""} disabled={locked}
              onChange={(event) => { if (event.target.value) void tutor.selectConversation(event.target.value); }}>
              <option value="">New conversation</option>
              {tutor.detail && !tutor.conversations.some((saved) => saved.id === tutor.detail?.conversation.id) ? <option value={tutor.detail.conversation.id}>{tutor.detail.conversation.title}</option> : null}
              {tutor.conversations.map((saved) => <option key={saved.id} value={saved.id}>{saved.title}</option>)}
            </select>
          </label>
          <Button size="small" onClick={() => tutor.newConversation()} disabled={locked}>New chat</Button>
          {tutor.detail ? <Button size="small" variant="dangerGhost" onClick={() => { void remove(); }} disabled={locked}>Delete</Button> : null}
        </div>
      ) : <p className={styles.notice}>Request one hint at a time. Source previews stay hidden. Close the panel to return to the same practice card.</p>}
      <details className={styles.options}>
        <summary>Material and model</summary>
        <div className={styles.toolbar}>
          {!hintOnly ? (
            <label className={styles.field} htmlFor={`${id}-scope`}>Study material
              <select id={`${id}-scope`} value={scopeKey} disabled={locked} onChange={(event) => {
                const selected = scopes.find((choice) => studyScopeKey(choice.scope) === event.target.value);
                if (selected) tutor.newConversation(selected.scope);
              }}>
                {!scopes.some((choice) => studyScopeKey(choice.scope) === scopeKey) ? <option value={scopeKey}>Selected material</option> : null}
                {scopes.map((choice) => <option key={studyScopeKey(choice.scope)} value={studyScopeKey(choice.scope)}>{choice.title}</option>)}
              </select>
            </label>
          ) : null}
          <label className={styles.field} htmlFor={`${id}-model`}>Tutor model
            <select id={`${id}-model`} value={tutor.profileId ?? ""} disabled={locked}
              onChange={(event) => tutor.setProfileId(event.target.value || null)}>
              <option value="">Configured Tutor default</option>
              {tutor.profileId && !profiles.some((profile) => profile.id === tutor.profileId) ? <option value={tutor.profileId}>Selected profile unavailable</option> : null}
              {profiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.displayName} — {profile.modelId}</option>)}
            </select>
          </label>
        </div>
        <p className={styles.muted}>Choose and test provider profiles in Settings → AI/LLM. General discussion uses the model without searching the library.</p>
      </details>
      {tutor.target ? <p className={styles.muted}>Current context: {tutor.target.title}</p> : null}
      {!profiles.length && !tutor.loading ? <p className={styles.notice}>Configure an AI provider in Settings → AI/LLM to send messages. Saved conversations remain readable without AI access.</p> : null}
      <div className={styles.transcript} role="log" aria-label="Saved conversation" aria-relevant="additions">
        {!turns.length ? <div className={styles.empty}><h3>{hintOnly ? "Work toward the answer" : "Ask about what you are studying"}</h3><p>Ask for an explanation, compare ideas, or request one quiz question. Responses are saved locally and may contain mistakes.</p></div> : null}
        {tutor.detail?.hasEarlierTurns && !hintOnly ? <Button disabled={locked} onClick={() => { void tutor.loadEarlier(); }}>Load earlier messages</Button> : null}
        {turns.map((turn) => (
          <article className={styles.turn} key={turn.id}>
            {turn.userText ? <section className={styles.userMessage} aria-label="Your message"><strong>You</strong><div className={styles.message}>{turn.userText}</div></section> : null}
            <section className={styles.assistantMessage} aria-label="Tutor response">
              <div className={styles.toolbar}><strong>Tutor</strong><span className={styles.muted}>{turn.model}</span></div>
              {turn.assistantText ? <div className={styles.message}>{turn.assistantText}</div> : null}
              {turn.state === "running" ? <p role="status">Response in progress…</p> : null}
              {turn.errorMessage ? <p role="alert">{turn.errorMessage}</p> : null}
              {!turn.evidenceCurrent ? <p className={styles.notice}>Source material changed or was removed. This historical response is not current evidence.</p> : null}
              {turn.feedback ? <details><summary>Feedback against the saved criteria</summary>{turn.feedback.criteriaFeedback.map((text, index) => <p key={index}>{index + 1}. {text}</p>)}</details> : null}
              {!hintOnly && turn.mode !== "quiz" ? <div className={styles.sources}>{turn.citations.map((citation, index) => <Button key={citation.id} size="small" variant="text" disabled={sourceBusy || !turn.evidenceCurrent} onClick={() => { void showSource(citation); }}>Source {index + 1}: {citation.title}</Button>)}</div> : null}
              {["failed", "canceled", "interrupted"].includes(turn.state) ? <Button size="small" disabled={locked} onClick={() => tutor.setDraft(turn.userText)}>Restore message to composer</Button> : null}
            </section>
          </article>
        ))}
      </div>
      {sourceError ? <p role="alert">{sourceError}</p> : null}
      {tutor.error ? <p className={styles.notice} role="alert">{tutor.error}</p> : null}
      {tutor.needsRefresh ? <Button disabled={tutor.busy} onClick={() => { void tutor.refresh(); }}>Refresh saved conversation</Button> : null}
      {tutor.pending ? <div className={styles.notice}><p>The request outcome needs checking. Retrying uses its original operation ID, not a second message.</p><Button disabled={tutor.busy} onClick={() => { void tutor.retryPending(); }}>Retry or check request</Button></div> : null}
      {tutor.busy || tutor.pending ? <div className={styles.toolbar}><span role="status">{hintOnly ? "Preparing a checked hint…" : "Tutor request in progress…"}</span><Button variant="dangerSecondary" onClick={() => { void tutor.cancel(); }}>Cancel response</Button></div> : null}
      <form className={styles.composer} onSubmit={(event) => { event.preventDefault(); submit(); }}>
        <label htmlFor={`${id}-message`}>{hintOnly ? "Hint request" : question ? "Your answer" : "Message"}</label>
        <textarea id={`${id}-message`} rows={4} maxLength={8000} value={tutor.draft} disabled={locked || tutor.needsRefresh}
          onChange={(event) => tutor.setDraft(event.target.value)} onKeyDown={(event) => {
            if (event.key === "Enter" && (event.ctrlKey || event.metaKey) && !event.nativeEvent.isComposing) { event.preventDefault(); submit(); }
          }} />
        <div className={styles.toolbar}>
          <Button type="submit" variant="primary" disabled={locked || tutor.needsRefresh || !tutor.draft.trim()}>{hintOnly ? "Request hint" : question ? "Submit answer" : "Send"}</Button>
          {!hintOnly ? <Button disabled={locked || tutor.needsRefresh} onClick={() => { void tutor.send("quiz", tutor.draft.trim() || "Ask me one question about the selected material."); }}>Quiz me</Button> : null}
          {!hintOnly && question ? <Button disabled={locked || !tutor.draft.trim()} onClick={() => { void tutor.send("explain"); }}>Ask for explanation instead</Button> : null}
        </div>
        <p className={styles.muted}>Sending shares this message and relevant study/conversation context with your selected AI provider. Provider charges and retention policies apply. The tutor never submits a card review or changes your schedule.</p>
      </form>
      {source && !hintOnly ? <Dialog title={source.title} onClose={() => setSource(null)} width="large"><p className={styles.message}>{source.excerpt}</p>{source.locator ? <p>Source units: {source.locator.unitNumbers.join(", ")}</p> : null}<p className={styles.muted}>Origin: {source.origin.replaceAll("_", " ")}. A source reference is not independent verification of every claim.</p></Dialog> : null}
    </section>
  );
}

export function TutorScreen() {
  const tutor = useTutor();
  useEffect(() => { void tutor.initialize(); }, [tutor.initialize]);
  return <div className={styles.screen} data-testid="tutor-screen"><header className={styles.toolbar}><h1>Tutor</h1><Button size="small" disabled={tutor.busy} onClick={() => { void tutor.initialize(); void tutor.refresh(); }}>Refresh</Button></header>{tutor.loading ? <p role="status">Loading saved conversations and model settings…</p> : null}<TutorWorkspace /></div>;
}

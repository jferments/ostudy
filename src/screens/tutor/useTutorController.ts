import { useCallback, useEffect, useRef, useState } from "react";

import { useRequestLane } from "../../controllers/requestLane";
import { getAiTaskDefaults, listAiProviderProfiles, type AiProviderProfileSummary } from "../../ipc/aiProfiles";
import { createOperationId, getErrorCode, getErrorMessage } from "../../ipc/common";
import { cancelLearningTask, listSemanticScopeChoices, type ScopeChoice, type StudyScope } from "../../ipc/knowledge";
import {
  createTutorConversation, deleteTutorConversation, getTutorConversation, listTutorConversations, sendTutorTurn,
  type ConversationDetail, type ConversationSummary, type TutorMode, type TutorTarget, type TutorTurnInput,
} from "../../ipc/tutor";
import { useMaterialPreparation } from "./useMaterialPreparation";

const GENERAL: StudyScope = { kind: "general", id: null };

/** A single mounted owner retains in-flight operations across route/panel changes. */
export function useTutorController() {
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [profiles, setProfiles] = useState<AiProviderProfileSummary[]>([]);
  const [choices, setChoices] = useState<ScopeChoice[]>([]);
  const [detail, setDetailState] = useState<ConversationDetail | null>(null);
  const current = useRef<ConversationDetail | null>(null);
  const [scope, setScope] = useState<StudyScope>(GENERAL);
  const [target, setTarget] = useState<TutorTarget | null>(null);
  const [panelOpen, setPanelOpen] = useState(false);
  const [draft, setDraft] = useState("");
  const [profileId, setProfileId] = useState<string | null>(null);
  const [defaultTutorProfileId, setDefaultTutorProfileId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [needsRefresh, setNeedsRefresh] = useState(false);
  const [pending, setPendingState] = useState<TutorTurnInput | null>(null);
  const pendingRef = useRef<TutorTurnInput | null>(null);
  const creating = useRef<Parameters<typeof createTutorConversation>[0] | null>(null);
  const active = useRef<string | null>(null);
  const mutating = useRef(false);
  const alive = useRef(true);
  const bootstrapLane = useRequestLane();
  const detailLane = useRequestLane();
  const historyLane = useRequestLane();
  const preparation = useMaterialPreparation(scope);

  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);

  const setDetail = useCallback((value: ConversationDetail | null) => {
    current.current = value;
    setDetailState(value);
  }, []);

  function setPending(value: TutorTurnInput | null): void {
    pendingRef.current = value;
    setPendingState(value);
  }

  const initialize = useCallback(async () => {
    const token = bootstrapLane.begin();
    setLoading(true);
    try {
      // Only local reads: entering Tutor never probes a provider or uploads a library.
      const [saved, available, materials, defaults] = await Promise.all([
        listTutorConversations(), listAiProviderProfiles(), listSemanticScopeChoices(), getAiTaskDefaults(),
      ]);
      if (!bootstrapLane.isCurrent(token)) return;
      setConversations(saved);
      setProfiles(available);
      setChoices(materials);
      // null is a live default assignment, not a copy of the old default ID.
      setDefaultTutorProfileId(defaults.tutorProfileId ?? null);
    } catch (failure: unknown) {
      if (bootstrapLane.isCurrent(token)) setError(getErrorMessage(failure));
    } finally {
      if (bootstrapLane.isCurrent(token)) setLoading(false);
    }
  }, [bootstrapLane]);

  async function refresh(): Promise<void> {
    const id = current.current?.conversation.id;
    if (!id || mutating.current) return;
    const token = detailLane.begin();
    try {
      const value = await getTutorConversation({ conversationId: id, beforeSequence: null });
      if (!detailLane.isCurrent(token) || current.current?.conversation.id !== id) return;
      setDetail(value);
      setNeedsRefresh(false);
      setError(null);
      const request = pendingRef.current;
      const turn = request && value.turns.find((entry) => entry.operationId === request.operationId);
      if (turn && turn.state !== "running") {
        setPending(null);
        active.current = null;
      }
    } catch (failure: unknown) {
      if (detailLane.isCurrent(token)) setError(getErrorMessage(failure));
    }
  }

  function canSwitch(): boolean {
    if (mutating.current || pendingRef.current || creating.current || preparation.locked) {
      setError("Resolve the current request before changing conversations. Closing the panel does not discard it.");
      return false;
    }
    return true;
  }

  async function selectConversation(id: string): Promise<void> {
    if (!canSwitch()) return;
    const token = detailLane.begin();
    setLoading(true);
    try {
      const value = await getTutorConversation({ conversationId: id, beforeSequence: null });
      if (!detailLane.isCurrent(token)) return;
      setDetail(value);
      setScope(value.conversation.scope);
      setTarget(null);
      setDraft("");
      setError(null);
      setNeedsRefresh(false);
    } catch (failure: unknown) {
      if (detailLane.isCurrent(token)) setError(getErrorMessage(failure));
    } finally {
      if (detailLane.isCurrent(token)) setLoading(false);
    }
  }

  function newConversation(next: StudyScope = scope): void {
    if (!canSwitch()) return;
    detailLane.invalidate();
    historyLane.invalidate();
    setLoading(false);
    setDetail(null);
    setScope(next);
    setTarget(null);
    setDraft("");
    setError(null);
    setNeedsRefresh(false);
  }

  function openContext(next: TutorTarget): void {
    if (!canSwitch()) return;
    setPanelOpen(true);
    newConversation(next.scope);
    setTarget(next);
    if (next.mode === "hint") setDraft("Give me a hint without revealing the answer.");
    void initialize();
  }

  async function perform(request: TutorTurnInput): Promise<void> {
    active.current = request.operationId;
    const turn = await sendTutorTurn(request);
    if (!alive.current) return;
    setPending(turn.state === "running" ? request : null);
    if (turn.state !== "running") {
      active.current = null;
      setDraft((text) => text === request.text ? "" : text);
    }
    const previous = current.current;
    if (previous?.conversation.id === request.conversationId) {
      const turns = [...previous.turns.filter((entry) => entry.id !== turn.id), turn];
      turns.sort((left, right) => left.sequence - right.sequence);
      setDetail({ ...previous, turns });
    }
    // The write is already acknowledged. A failed following read must never
    // turn this into a failed submission or allocate another operation ID.
    try {
      const value = await getTutorConversation({ conversationId: request.conversationId, beforeSequence: null });
      if (!alive.current) return;
      setDetail(value);
      setNeedsRefresh(false);
      setError(null);
      setConversations((saved) => [value.conversation, ...saved.filter((entry) => entry.id !== value.conversation.id)]);
    } catch (failure: unknown) {
      if (!alive.current) return;
      setNeedsRefresh(true);
      setError(`Response state saved; refresh failed: ${getErrorMessage(failure)}`);
    }
  }

  async function send(mode: TutorMode, text = draft): Promise<void> {
    if (mutating.current || loading || pendingRef.current || needsRefresh || preparation.locked) return;
    if (!text.trim() && mode !== "quiz") return;
    mutating.current = true;
    detailLane.invalidate();
    setBusy(true);
    setError(null);
    let request: TutorTurnInput | null = null;
    try {
      let value = current.current;
      if (!value) {
        creating.current ??= {
          operationId: createOperationId(), scope,
          title: (target?.title || text.trim() || "Study conversation").slice(0, 200),
        };
        const conversation = await createTutorConversation(creating.current);
        creating.current = null;
        value = { conversation, turns: [], hasEarlierTurns: false };
        setDetail(value);
      }
      const question = mode === "answer" ? [...value.turns].reverse().find((turn) => turn.pendingQuestion) : null;
      if (mode === "answer" && !question) throw new Error("Ask for a quiz question before submitting an answer.");
      request = {
        operationId: createOperationId(), conversationId: value.conversation.id,
        expectedRevision: value.conversation.revision, mode, text,
        focus: target?.focus ?? null, profileId, includeConversations: false,
        questionId: question?.id ?? null,
      };
      await perform(request);
    } catch (failure: unknown) {
      if (!alive.current) return;
      const code = getErrorCode(failure);
      const rejectedBeforeAdmission = code !== null && (
        code.startsWith("invalid_") || ["tutor_focus_changed", "tutor_answer_not_revealed", "ai_task_profile_required",
          "ai_profile_disabled", "ai_document_model_not_verified", "tutor_conversation_changed",
          "learning_task_running", "learning_task_capacity", "tutor_conversation_missing"].includes(code)
      );
      if (request && !rejectedBeforeAdmission) setPending(request);
      else active.current = null;
      if (code === "tutor_conversation_changed") setNeedsRefresh(true);
      setError(getErrorMessage(failure));
    } finally {
      mutating.current = false;
      if (alive.current) setBusy(false);
    }
  }

  async function retryPending(): Promise<void> {
    const request = pendingRef.current;
    if (!request || mutating.current) return;
    mutating.current = true;
    setBusy(true);
    try { await perform(request); }
    catch (failure: unknown) { if (alive.current) setError(getErrorMessage(failure)); }
    finally { mutating.current = false; if (alive.current) setBusy(false); }
  }

  async function cancel(): Promise<void> {
    if (!active.current) return;
    try { await cancelLearningTask(active.current); }
    catch (failure: unknown) { if (alive.current) setError(getErrorMessage(failure)); }
  }

  async function loadEarlier(): Promise<void> {
    const value = current.current;
    if (!value?.hasEarlierTurns || mutating.current) return;
    const token = historyLane.begin();
    try {
      const page = await getTutorConversation({ conversationId: value.conversation.id, beforeSequence: value.turns[0]?.sequence ?? null });
      if (!historyLane.isCurrent(token) || current.current?.conversation.id !== value.conversation.id) return;
      const latest = current.current;
      const ids = new Set(latest.turns.map((turn) => turn.id));
      setDetail({ ...latest, turns: [...page.turns.filter((turn) => !ids.has(turn.id)), ...latest.turns], hasEarlierTurns: page.hasEarlierTurns });
    } catch (failure: unknown) { if (historyLane.isCurrent(token)) setError(getErrorMessage(failure)); }
  }

  async function removeConversation(): Promise<void> {
    const value = current.current;
    if (!value || !canSwitch()) return;
    mutating.current = true;
    detailLane.invalidate();
    setBusy(true);
    try {
      await deleteTutorConversation({ conversationId: value.conversation.id, expectedRevision: value.conversation.revision });
      setDetail(null);
      setConversations((saved) => saved.filter((entry) => entry.id !== value.conversation.id));
      setDraft("");
      setError(null);
    } catch (failure: unknown) { setError(getErrorMessage(failure)); }
    finally { mutating.current = false; setBusy(false); }
  }

  return {
    conversations, profiles, choices, detail, scope, target, panelOpen, draft, profileId, defaultTutorProfileId,
    loading, busy, error, pending, needsRefresh, preparation, initialize, refresh, selectConversation,
    newConversation, openContext, closePanel: () => setPanelOpen(false), setDraft, setProfileId,
    send, retryPending, cancel, loadEarlier, removeConversation,
  };
}

export type TutorController = ReturnType<typeof useTutorController>;

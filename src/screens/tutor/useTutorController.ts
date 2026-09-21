import { useCallback, useEffect, useRef, useState } from "react";

import { useRequestLane } from "../../controllers/requestLane";
import { getAiTaskDefaults, listAiProviderProfiles, type AiProviderProfileSummary } from "../../ipc/aiProfiles";
import { createOperationId, getErrorCode, getErrorMessage } from "../../ipc/common";
import { cancelLearningTask, listSemanticScopeChoices, type ScopeChoice, type StudyScope } from "../../ipc/knowledge";
import {
  createTutorConversation,
  createTutorProject,
  createTutorProjectConversation,
  deleteTutorConversation,
  deleteTutorProject,
  getTutorConversation,
  listTutorConversations,
  listTutorProjects,
  moveTutorConversation,
  renameTutorConversation,
  sendTutorTurn,
  setTutorConversationArchived,
  setTutorProjectArchived,
  updateTutorProject,
  type ConversationDetail,
  type ConversationSummary,
  type TutorMode,
  type TutorProject,
  type TutorTarget,
  type TutorTurn,
  type TutorTurnInput,
} from "../../ipc/tutor";
import { useMaterialPreparation } from "./useMaterialPreparation";

const GENERAL: StudyScope = { kind: "general", id: null };

function scopeUsesCollection(scope: StudyScope): boolean {
  return scope.kind === "collection"
    || (scope.kind === "selection" && scope.members.some((member) => member.kind === "collection"));
}

interface PendingConversationCreate {
  operationId: string;
  scope: StudyScope;
  title: string;
  projectId: string | null;
  projectScopeOverride: StudyScope | null;
}

export interface TutorProjectValues {
  title: string;
  description: string;
  instructions: string;
  defaultScope: StudyScope;
}

function orderConversations(values: ConversationSummary[]): ConversationSummary[] {
  return [...values].sort((left, right) => {
    const archive = Number(left.archivedAt !== null) - Number(right.archivedAt !== null);
    return archive || right.updatedAt - left.updatedAt || left.id.localeCompare(right.id);
  });
}

function orderProjects(values: TutorProject[]): TutorProject[] {
  return [...values].sort((left, right) => {
    const archive = Number(left.archivedAt !== null) - Number(right.archivedAt !== null);
    return archive || left.title.localeCompare(right.title) || left.id.localeCompare(right.id);
  });
}

/** A single mounted owner retains in-flight operations across route/panel changes. */
export function useTutorController() {
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [projects, setProjects] = useState<TutorProject[]>([]);
  const [profiles, setProfiles] = useState<AiProviderProfileSummary[]>([]);
  const [choices, setChoices] = useState<ScopeChoice[]>([]);
  const [detail, setDetailState] = useState<ConversationDetail | null>(null);
  const current = useRef<ConversationDetail | null>(null);
  const [scope, setScope] = useState<StudyScope>(GENERAL);
  const [draftProjectId, setDraftProjectId] = useState<string | null>(null);
  const [projectScopeOverride, setProjectScopeOverride] = useState<StudyScope | null>(null);
  const [target, setTarget] = useState<TutorTarget | null>(null);
  const [includeConversations, setConversationInclusion] = useState(false);
  const [panelOpen, setPanelOpen] = useState(false);
  const [draft, setDraft] = useState("");
  const [profileId, setProfileId] = useState<string | null>(null);
  const [defaultTutorProfileId, setDefaultTutorProfileId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [organizing, setOrganizing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [needsRefresh, setNeedsRefresh] = useState(false);
  const [pending, setPendingState] = useState<TutorTurnInput | null>(null);
  const pendingRef = useRef<TutorTurnInput | null>(null);
  const creating = useRef<PendingConversationCreate | null>(null);
  const organizationOperations = useRef(new Map<string, string>());
  const active = useRef<string | null>(null);
  const mutating = useRef(false);
  const organizationMutating = useRef(false);
  const alive = useRef(true);
  const bootstrapLane = useRequestLane();
  const detailLane = useRequestLane();
  const historyLane = useRequestLane();
  const conversationRetrievalAllowed = scope.kind !== "general" && !scopeUsesCollection(scope) && target?.mode !== "hint";
  const preparation = useMaterialPreparation(scope, includeConversations && conversationRetrievalAllowed);

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

  function applyConversationSummary(value: ConversationSummary): void {
    setConversations((saved) => orderConversations([value, ...saved.filter((entry) => entry.id !== value.id)]));
    const selected = current.current;
    if (selected?.conversation.id === value.id) {
      setDetail({ ...selected, conversation: value });
      setScope(value.scope);
    }
  }

  function applyProjectSummary(value: TutorProject): void {
    setProjects((saved) => orderProjects([value, ...saved.filter((entry) => entry.id !== value.id)]));
  }

  function operationFor(key: string): string {
    const existing = organizationOperations.current.get(key);
    if (existing) return existing;
    const operationId = createOperationId();
    organizationOperations.current.set(key, operationId);
    return operationId;
  }

  function organizationFailureIsFinal(code: string | null): boolean {
    return code !== null && (code.startsWith("invalid_") || [
      "tutor_project_changed",
      "tutor_project_missing",
      "tutor_conversation_changed",
      "tutor_conversation_organization_changed",
      "tutor_conversation_missing",
      "tutor_turn_running",
    ].includes(code));
  }

  const initialize = useCallback(async () => {
    const token = bootstrapLane.begin();
    setLoading(true);
    try {
      // Only local reads: entering Tutor never probes a provider or uploads a library.
      const [saved, savedProjects, available, materials, defaults] = await Promise.all([
        listTutorConversations(), listTutorProjects(), listAiProviderProfiles(), listSemanticScopeChoices(), getAiTaskDefaults(),
      ]);
      if (!bootstrapLane.isCurrent(token)) return;
      setConversations(orderConversations(saved));
      setProjects(orderProjects(savedProjects));
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
        settleDraft(turn.state, request?.text ?? turn.userText);
      }
    } catch (failure: unknown) {
      if (detailLane.isCurrent(token)) setError(getErrorMessage(failure));
    }
  }

  function canSwitch(): boolean {
    if (mutating.current || organizationMutating.current || pendingRef.current || creating.current || preparation.locked) {
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
      setDraftProjectId(null);
      setProjectScopeOverride(null);
      setConversationInclusion(false);
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
    setDraftProjectId(null);
    setProjectScopeOverride(null);
    setConversationInclusion(false);
    setTarget(null);
    setDraft("");
    setError(null);
    setNeedsRefresh(false);
  }

  function newConversationInProject(projectId: string): void {
    if (!canSwitch()) return;
    const project = projects.find((entry) => entry.id === projectId && entry.archivedAt === null);
    if (!project) {
      setError("Restore this Tutor project before starting a new chat in it.");
      return;
    }
    detailLane.invalidate();
    historyLane.invalidate();
    setLoading(false);
    setDetail(null);
    setScope(project.defaultScope);
    setDraftProjectId(project.id);
    setProjectScopeOverride(null);
    setConversationInclusion(false);
    setTarget(null);
    setDraft("");
    setError(null);
    setNeedsRefresh(false);
  }

  function changeSources(next: StudyScope): void {
    if (!canSwitch()) return;
    const projectId = draftProjectId ?? current.current?.conversation.projectId ?? null;
    const text = draft;
    detailLane.invalidate();
    historyLane.invalidate();
    setLoading(false);
    setDetail(null);
    setScope(next);
    setDraftProjectId(projectId);
    setProjectScopeOverride(projectId ? next : null);
    setConversationInclusion(false);
    setTarget(null);
    setDraft(text);
    setError(null);
    setNeedsRefresh(false);
  }

  function settleDraft(state: TutorTurn["state"], text: string): void {
    if (state === "complete") setDraft((value) => value === text ? "" : value);
    else if (state !== "running") setDraft((value) => value || text);
  }

  async function retryTurn(turn: TutorTurn): Promise<void> {
    const saved = current.current?.turns.at(-1);
    if (!saved || saved.id !== turn.id || !["failed", "canceled", "interrupted"].includes(saved.state)) {
      setError("Only the latest failed response can be retried. Send a new message for an earlier question.");
      return;
    }
    if (saved.mode === "hint" && target?.mode !== "hint") {
      setError("Reopen the current Practice card to request a hint.");
      return;
    }
    // Confirmed terminal failure is a new intent; an ambiguous acknowledgement
    // uses retryPending with the original identity. Older answers cannot silently
    // become answers to a newer pending quiz.
    await send(saved.mode, saved.userText);
  }

  function openContext(next: TutorTarget): void {
    if (!canSwitch()) return;
    setPanelOpen(true);
    newConversation(next.scope);
    setTarget(next);
    if (next.mode === "hint") setDraft("Give me a hint without revealing the answer.");
    void initialize();
  }

  function setIncludeConversations(enabled: boolean): void {
    if (loading || !canSwitch()) return;
    // This opts into retrieval for the current interaction only. It neither
    // grants a conversation sharing permission nor starts a provider request.
    setConversationInclusion(enabled && conversationRetrievalAllowed);
  }

  async function perform(request: TutorTurnInput): Promise<void> {
    active.current = request.operationId;
    const turn = await sendTutorTurn(request);
    if (!alive.current) return;
    setPending(turn.state === "running" ? request : null);
    if (turn.state !== "running") {
      active.current = null;
      settleDraft(turn.state, request.text);
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
          operationId: createOperationId(),
          scope,
          title: (target?.title || text.trim() || "Study conversation").slice(0, 200),
          projectId: draftProjectId,
          projectScopeOverride,
        };
        const planned = creating.current;
        const conversation = planned.projectId
          ? await createTutorProjectConversation({
              operationId: planned.operationId,
              projectId: planned.projectId,
              title: planned.title,
              scope: planned.projectScopeOverride,
            })
          : await createTutorConversation({
              operationId: planned.operationId,
              scope: planned.scope,
              title: planned.title,
            });
        creating.current = null;
        setDraftProjectId(null);
        setProjectScopeOverride(null);
        setScope(conversation.scope);
        setConversations((saved) => orderConversations([
          conversation,
          ...saved.filter((entry) => entry.id !== conversation.id),
        ]));
        value = { conversation, turns: [], hasEarlierTurns: false };
        setDetail(value);
      }
      const question = mode === "answer" ? [...value.turns].reverse().find((turn) => turn.pendingQuestion) : null;
      if (mode === "answer" && !question) throw new Error("Ask for a quiz question before submitting an answer.");
      request = {
        operationId: createOperationId(), conversationId: value.conversation.id,
        expectedRevision: value.conversation.revision, mode, text,
        focus: target?.focus ?? null, profileId,
        includeConversations: includeConversations && conversationRetrievalAllowed && mode !== "hint",
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

  async function runOrganization<T>(
    key: string,
    mutation: (operationId: string) => Promise<T>,
    apply: (value: T) => void,
  ): Promise<boolean> {
    if (organizationMutating.current) return false;
    organizationMutating.current = true;
    setOrganizing(true);
    setError(null);
    const operationId = operationFor(key);
    try {
      const value = await mutation(operationId);
      if (!alive.current) return false;
      organizationOperations.current.delete(key);
      apply(value);
      return true;
    } catch (failure: unknown) {
      if (!alive.current) return false;
      const code = getErrorCode(failure);
      if (organizationFailureIsFinal(code)) organizationOperations.current.delete(key);
      setError(getErrorMessage(failure));
      return false;
    } finally {
      organizationMutating.current = false;
      if (alive.current) setOrganizing(false);
    }
  }

  async function createProject(values: TutorProjectValues): Promise<boolean> {
    const key = `create-project:${JSON.stringify(values)}`;
    return runOrganization(
      key,
      (operationId) => createTutorProject({ operationId, ...values }),
      applyProjectSummary,
    );
  }

  async function updateProject(project: TutorProject, values: TutorProjectValues): Promise<boolean> {
    const key = `update-project:${project.id}:${project.revision}:${JSON.stringify(values)}`;
    return runOrganization(
      key,
      (operationId) => updateTutorProject({
        operationId,
        projectId: project.id,
        expectedRevision: project.revision,
        ...values,
      }),
      applyProjectSummary,
    );
  }

  async function setProjectArchived(project: TutorProject, archived: boolean): Promise<boolean> {
    const key = `archive-project:${project.id}:${project.revision}:${archived}`;
    return runOrganization(
      key,
      (operationId) => setTutorProjectArchived({
        operationId,
        projectId: project.id,
        expectedRevision: project.revision,
        archived,
      }),
      applyProjectSummary,
    );
  }

  async function removeProject(project: TutorProject): Promise<boolean> {
    const key = `delete-project:${project.id}:${project.revision}`;
    return runOrganization(
      key,
      (operationId) => deleteTutorProject({
        operationId,
        projectId: project.id,
        expectedRevision: project.revision,
      }),
      () => {
        setProjects((saved) => saved.filter((entry) => entry.id !== project.id));
        const detach = (entry: ConversationSummary): ConversationSummary => entry.projectId === project.id
          ? { ...entry, projectId: null, organizationRevision: entry.organizationRevision + 1 }
          : entry;
        setConversations((saved) => orderConversations(saved.map(detach)));
        const selected = current.current;
        if (selected?.conversation.projectId === project.id) {
          setDetail({ ...selected, conversation: detach(selected.conversation) });
        }
        if (draftProjectId === project.id) {
          setDraftProjectId(null);
          setProjectScopeOverride(null);
        }
      },
    );
  }

  async function renameConversation(conversation: ConversationSummary, title: string): Promise<boolean> {
    const normalized = title.trim();
    const key = `rename-conversation:${conversation.id}:${conversation.organizationRevision}:${normalized}`;
    return runOrganization(
      key,
      (operationId) => renameTutorConversation({
        operationId,
        conversationId: conversation.id,
        expectedOrganizationRevision: conversation.organizationRevision,
        title: normalized,
      }),
      applyConversationSummary,
    );
  }

  async function moveConversation(
    conversation: ConversationSummary,
    projectId: string | null,
    applyProjectDefault: boolean,
  ): Promise<boolean> {
    const key = `move-conversation:${conversation.id}:${conversation.organizationRevision}:${projectId ?? "ungrouped"}:${applyProjectDefault}`;
    return runOrganization(
      key,
      (operationId) => moveTutorConversation({
        operationId,
        conversationId: conversation.id,
        expectedOrganizationRevision: conversation.organizationRevision,
        projectId,
        applyProjectDefault,
      }),
      applyConversationSummary,
    );
  }

  async function setConversationArchived(conversation: ConversationSummary, archived: boolean): Promise<boolean> {
    const key = `archive-conversation:${conversation.id}:${conversation.organizationRevision}:${archived}`;
    return runOrganization(
      key,
      (operationId) => setTutorConversationArchived({
        operationId,
        conversationId: conversation.id,
        expectedOrganizationRevision: conversation.organizationRevision,
        archived,
      }),
      applyConversationSummary,
    );
  }

  async function removeConversation(conversation = current.current?.conversation ?? null): Promise<boolean> {
    if (!conversation) return false;
    const key = `delete-conversation:${conversation.id}:${conversation.revision}:${conversation.organizationRevision}`;
    return runOrganization(
      key,
      (operationId) => deleteTutorConversation({
        operationId,
        conversationId: conversation.id,
        expectedRevision: conversation.revision,
        expectedOrganizationRevision: conversation.organizationRevision,
      }),
      () => {
        setConversations((saved) => saved.filter((entry) => entry.id !== conversation.id));
        if (current.current?.conversation.id === conversation.id) {
          setDetail(null);
          setDraftProjectId(null);
          setProjectScopeOverride(null);
          setDraft("");
        }
      },
    );
  }

  return {
    conversations, projects, profiles, choices, detail, scope, draftProjectId, target, panelOpen, draft, profileId, defaultTutorProfileId,
    loading, busy, organizing, error, pending, needsRefresh, preparation, includeConversations, conversationRetrievalAllowed, setIncludeConversations,
    initialize, refresh, selectConversation,
    newConversation, newConversationInProject, changeSources, retryTurn, openContext, closePanel: () => setPanelOpen(false), setDraft, setProfileId,
    send, retryPending, cancel, loadEarlier,
    createProject, updateProject, setProjectArchived, removeProject,
    renameConversation, moveConversation, setConversationArchived, removeConversation,
  };
}

export type TutorController = ReturnType<typeof useTutorController>;

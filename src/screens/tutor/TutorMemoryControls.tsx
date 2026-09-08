import { useEffect, useRef, useState } from "react";

import { useRequestLane } from "../../controllers/requestLane";
import { createOperationId, getErrorCode, getErrorMessage } from "../../ipc/common";
import { changeTutorMemory, getTutorMemory, type ChangeTutorMemoryInput, type MemoryAction, type TutorMemory } from "../../ipc/tutorMemory";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import styles from "./TutorWorkspace.module.css";
import memoryStyles from "./TutorMemoryControls.module.css";

interface Props {
  conversationId: string;
  revision: number;
  retrievalAvailable: boolean;
  disabled: boolean;
  onBusyChange: (busy: boolean) => void;
  onChanged: () => Promise<unknown>;
}

export function TutorMemoryControls({ conversationId, revision, retrievalAvailable, disabled, onBusyChange, onChanged }: Props) {
  const [memory, setMemory] = useState<TutorMemory | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<ChangeTutorMemoryInput | null>(null);
  const [busy, setBusy] = useState(false);
  const mutation = useRef(false);
  const alive = useRef(true);
  const lane = useRequestLane();
  const confirm = useConfirm();
  const busyCallback = useRef(onBusyChange);
  busyCallback.current = onBusyChange;

  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; busyCallback.current(false); };
  }, []);
  useEffect(() => { busyCallback.current(busy || pending !== null); }, [busy, pending]);
  useEffect(() => {
    const token = lane.begin();
    void getTutorMemory(conversationId).then((value) => {
      if (lane.isCurrent(token)) { setMemory(value); setError(null); }
    }).catch((failure: unknown) => {
      if (lane.isCurrent(token)) setError(getErrorMessage(failure));
    });
  }, [conversationId, revision, lane]);

  async function change(action: MemoryAction, replay?: ChangeTutorMemoryInput): Promise<void> {
    if (disabled || mutation.current || (!memory && !replay) || (pending && !replay)) return;
    mutation.current = true;
    setBusy(true);
    setError(null);
    lane.invalidate();
    const input = replay ?? { operationId: createOperationId(), conversationId, expectedRevision: memory!.revision, action };
    try {
      const saved = await changeTutorMemory(input);
      if (!alive.current) return;
      setMemory(saved);
      setPending(null);
      // Persistence already succeeded. Refresh trouble must not replay a privacy action.
      try { await onChanged(); }
      catch (failure: unknown) {
        if (alive.current) setError(`Memory settings were saved; refresh the conversation: ${getErrorMessage(failure)}`);
      }
    } catch (failure: unknown) {
      if (!alive.current) return;
      const code = getErrorCode(failure);
      const rejected = code !== null && (code.startsWith("invalid_") || [
        "tutor_conversation_changed", "tutor_conversation_missing", "tutor_turn_running", "learning_maintenance_busy",
      ].includes(code));
      setPending(rejected ? null : input);
      setError(getErrorMessage(failure));
      if (rejected) {
        try { const value = await getTutorMemory(conversationId); if (alive.current) setMemory(value); } catch { /* Keep original rejection visible. */ }
      }
    } finally {
      mutation.current = false;
      if (alive.current) setBusy(false);
    }
  }

  async function forget(): Promise<void> {
    if (await confirm({
      title: "Forget earlier conversation context?",
      description: "Earlier messages stay readable, but will no longer enter future summaries or semantic retrieval. This also retires earlier quiz questions. It cannot erase existing backups or data already sent to your provider.",
      confirmLabel: "Forget earlier context", destructive: true,
    })) await change("forget_earlier");
  }

  const locked = disabled || busy || pending !== null || memory === null;
  return <details className={styles.options}>
    <summary>Conversation memory</summary>
    {error ? <p role="alert">{error}</p> : null}
    {memory ? <>
      <label className={memoryStyles.toggle}>
        <input type="checkbox" checked={memory.historyEnabled} disabled={locked}
          onChange={(event) => { void change(event.target.checked ? "enable_history" : "disable_history"); }} />
        Use earlier turns automatically in this conversation
      </label>
      {retrievalAvailable ? <label className={memoryStyles.toggle}>
        <input type="checkbox" checked={memory.retrievalEnabled} disabled={locked}
          onChange={(event) => { void change(event.target.checked ? "allow_retrieval" : "deny_retrieval"); }} />
        Allow this conversation in semantic retrieval
      </label> : null}
      <p className={styles.muted}>{retrievalAvailable ? "Retrieval is separate from automatic history and is limited to this material scope. Enabling it does not send data: explicitly prepare conversations before searching them. Disabling it removes existing retrieval projections." : "General chats use automatic history, not library retrieval. Use a material-scoped conversation to retrieve related chat messages semantically."}</p>
      <Button disabled={locked} onClick={() => { void forget(); }}>Forget earlier context</Button>
      {memory.startSequence > 0 ? <p className={styles.muted}>Turns through {memory.startSequence} are excluded from future memory.</p> : null}
      <details><summary>Stored conversation summary</summary>
        <p className={styles.message}>{memory.summary || "No stored summary. Recent eligible messages may still be used when automatic history is enabled."}</p>
        {memory.summary ? <p className={styles.muted}>Covers eligible turns through {memory.summaryThroughSequence}. This is a fallible conversation summary, not a learning profile or verified source.</p> : null}
      </details>
    </> : <p role="status">Loading local memory settings…</p>}
    {pending ? <Button disabled={disabled || busy} onClick={() => { void change(pending.action, pending); }}>Check memory change result</Button> : null}
  </details>;
}

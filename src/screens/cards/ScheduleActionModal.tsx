import { useRef, useState } from "react";

import {
  applyScheduleAction,
  createMutationId,
  getErrorMessage,
  type SelectionCard,
} from "../../ipc";

interface ScheduleActionModalProps {
  cards: SelectionCard[];
  action: "reset" | "reschedule";
  onClose: () => void;
  onComplete: () => Promise<void>;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

export function ScheduleActionModal({ cards, action, onClose, onComplete, showToast }: ScheduleActionModalProps) {
  const [dueChoice, setDueChoice] = useState("tomorrow");
  const [customDate, setCustomDate] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const savingRef = useRef(false);

  async function submit(): Promise<void> {
    if (savingRef.current) return;
    const dueAt = action === "reschedule" ? resolveDueAt(dueChoice, customDate) : undefined;
    if (action === "reschedule" && dueAt === null) {
      setErrorMessage("Choose a valid custom due date.");
      return;
    }
    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      for (const card of cards) {
        await applyScheduleAction({
          mutationId: createMutationId(),
          cardIds: [card.cardId],
          action,
          dueAt: dueAt ?? undefined,
        });
      }
      await onComplete();
      showToast(
        "success",
        action === "reset"
          ? `Reset ${cards.length} ${cards.length === 1 ? "card" : "cards"} to New.`
          : `Rescheduled ${cards.length} ${cards.length === 1 ? "card" : "cards"}.`,
      );
      onClose();
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      savingRef.current = false;
      setIsSaving(false);
    }
  }

  return (
    <div className="modal-backdrop schedule-action-dialog" role="presentation" onMouseDown={(event) => {
      if (event.currentTarget === event.target && !isSaving) onClose();
    }}>
      <section className="modal-card" role="dialog" aria-modal="true" aria-labelledby="schedule-action-heading">
        <header className="modal-header">
          <div><h2 id="schedule-action-heading">{action === "reset" ? "Reset schedule" : "Reschedule cards"}</h2><p>{cards.length} {cards.length === 1 ? "card" : "cards"} selected</p></div>
          <button type="button" className="text-button" disabled={isSaving} onClick={onClose}>Close</button>
        </header>
        {action === "reset" ? (
          <div className="inline-alert warning-alert">The selected cards will become New and due now. Existing review history remains available for later analytics.</div>
        ) : (
          <div className="schedule-action-options">
            <label><span>New due date</span><select value={dueChoice} onChange={(event) => setDueChoice(event.currentTarget.value)}>
              <option value="today">Today</option><option value="tomorrow">Tomorrow</option><option value="3_days">In 3 days</option><option value="1_week">In 1 week</option><option value="custom">Custom date</option>
            </select></label>
            {dueChoice === "custom" ? <label><span>Custom date</span><input type="date" value={customDate} onChange={(event) => setCustomDate(event.currentTarget.value)} /></label> : null}
            <p className="muted-copy">Rescheduling changes the next due date without recording a successful review or changing FSRS difficulty and stability.</p>
          </div>
        )}
        {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
        <footer className="modal-actions">
          <button type="button" className="secondary-button" disabled={isSaving} onClick={onClose}>Cancel</button>
          <button type="button" className={action === "reset" ? "danger-button" : "primary-button"} disabled={isSaving} onClick={() => void submit()}>{isSaving ? "Applying…" : action === "reset" ? "Reset schedule" : "Reschedule"}</button>
        </footer>
      </section>
    </div>
  );
}

function resolveDueAt(choice: string, customDate: string): number | null {
  const date = new Date();
  date.setHours(12, 0, 0, 0);
  if (choice === "today") return date.getTime();
  if (choice === "tomorrow") date.setDate(date.getDate() + 1);
  else if (choice === "3_days") date.setDate(date.getDate() + 3);
  else if (choice === "1_week") date.setDate(date.getDate() + 7);
  else if (choice === "custom") {
    const parsed = new Date(`${customDate}T12:00:00`);
    return Number.isNaN(parsed.getTime()) ? null : parsed.getTime();
  }
  return date.getTime();
}

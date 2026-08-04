import { useRef, useState } from "react";

import {
  applyScheduleAction,
  createMutationId,
  getErrorCode,
  getErrorMessage,
  type SelectionCard,
} from "../../ipc";
import { scheduleActionNotice } from "./scheduleActionResult";

interface ScheduleActionModalProps {
  cards: SelectionCard[];
  action: "reset" | "reschedule";
  onClose: () => void;
  onComplete: () => Promise<void>;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

interface PendingScheduleMutation {
  key: string;
  mutationId: string;
}

export function ScheduleActionModal({
  cards,
  action,
  onClose,
  onComplete,
  showToast,
}: ScheduleActionModalProps) {
  const [dueChoice, setDueChoice] = useState("tomorrow");
  const [customDate, setCustomDate] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const savingRef = useRef(false);
  const pendingMutationRef = useRef<PendingScheduleMutation | null>(null);

  async function submit(): Promise<void> {
    if (savingRef.current) return;
    const dueAt = action === "reschedule" ? resolveDueAt(dueChoice, customDate) : undefined;
    if (action === "reschedule" && dueAt === null) {
      pendingMutationRef.current = null;
      setErrorMessage("Choose a valid custom due date.");
      return;
    }
    const cardIds = [...new Set(cards.map((card) => card.cardId))].sort();
    const key = JSON.stringify({ action, cardIds, dueAt });
    const mutationId = mutationIdForRequest(key);

    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);

    let result;
    try {
      result = await applyScheduleAction({
        mutationId,
        cardIds,
        action,
        dueAt: dueAt ?? undefined,
      });
      pendingMutationRef.current = null;
    } catch (error: unknown) {
      if (getErrorCode(error) !== null) {
        pendingMutationRef.current = null;
        setErrorMessage(getErrorMessage(error));
      } else {
        setErrorMessage(
          `${getErrorMessage(error)} Choose the same action again to safely retry the complete selection.`,
        );
      }
      savingRef.current = false;
      setIsSaving(false);
      return;
    }

    const notice = scheduleActionNotice(result, cardIds.length);
    showToast(notice.kind, notice.text);
    try {
      await onComplete();
    } catch (error: unknown) {
      showToast(
        "info",
        `The schedule changes were saved, but the current view could not refresh: ${getErrorMessage(error)}`,
      );
    } finally {
      savingRef.current = false;
      setIsSaving(false);
      close();
    }
  }

  function mutationIdForRequest(key: string): string {
    if (pendingMutationRef.current?.key === key) {
      return pendingMutationRef.current.mutationId;
    }
    const mutationId = createMutationId();
    pendingMutationRef.current = { key, mutationId };
    return mutationId;
  }

  function close(): void {
    if (savingRef.current) return;
    pendingMutationRef.current = null;
    onClose();
  }

  return (
    <div
      className="modal-backdrop schedule-action-dialog"
      role="presentation"
      onMouseDown={(event) => {
        if (event.currentTarget === event.target) close();
      }}
    >
      <section
        className="modal-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="schedule-action-heading"
      >
        <header className="modal-header">
          <div>
            <h2 id="schedule-action-heading">
              {action === "reset" ? "Reset schedule" : "Reschedule cards"}
            </h2>
            <p>
              {cards.length} {cards.length === 1 ? "card" : "cards"} selected
            </p>
          </div>
          <button
            type="button"
            className="text-button"
            disabled={isSaving}
            onClick={close}
          >
            Close
          </button>
        </header>
        {action === "reset" ? (
          <div className="inline-alert warning-alert">
            The selected cards will become New and due now. Existing review history
            remains available for later analytics. The complete selection changes in
            one transaction or not at all.
          </div>
        ) : (
          <div className="schedule-action-options">
            <label>
              <span>New due date</span>
              <select
                value={dueChoice}
                disabled={isSaving}
                onChange={(event) => {
                  pendingMutationRef.current = null;
                  setDueChoice(event.currentTarget.value);
                }}
              >
                <option value="today">Today</option>
                <option value="tomorrow">Tomorrow</option>
                <option value="3_days">In 3 days</option>
                <option value="1_week">In 1 week</option>
                <option value="custom">Custom date</option>
              </select>
            </label>
            {dueChoice === "custom" ? (
              <label>
                <span>Custom date</span>
                <input
                  type="date"
                  value={customDate}
                  disabled={isSaving}
                  onChange={(event) => {
                    pendingMutationRef.current = null;
                    setCustomDate(event.currentTarget.value);
                  }}
                />
              </label>
            ) : null}
            <p className="muted-copy">
              Rescheduling changes the next due date without recording a successful
              review or changing FSRS difficulty and stability. The complete selection
              changes in one transaction or not at all.
            </p>
          </div>
        )}
        {errorMessage ? (
          <div className="inline-alert error-alert">{errorMessage}</div>
        ) : null}
        <footer className="modal-actions">
          <button
            type="button"
            className="secondary-button"
            disabled={isSaving}
            onClick={close}
          >
            Cancel
          </button>
          <button
            type="button"
            className={action === "reset" ? "danger-button" : "primary-button"}
            disabled={isSaving}
            onClick={() => void submit()}
          >
            {isSaving
              ? "Applying…"
              : action === "reset"
                ? "Reset schedule"
                : "Reschedule"}
          </button>
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

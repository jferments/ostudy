import { useRef, useState } from "react";
import type { SelectionCard } from "../../ipc/cards";
import { createMutationId, getErrorCode, getErrorMessage } from "../../ipc/common";
import { applyScheduleAction, type RescheduleIntent } from "../../ipc/scheduling";
import { Button } from "../../ui/primitives/Button";
import { TextField } from "../../ui/primitives/TextField";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Dialog } from "../../ui/overlays/Dialog";
import { scheduleActionNotice } from "./scheduleActionResult";
import styles from "./browserPresentation";

interface ScheduleActionModalProps {
  cards: SelectionCard[];
  action: "reset" | "reschedule";
  onClose: () => void;
  onComplete: () => Promise<void>;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}
interface PendingScheduleMutation { key: string; mutationId: string; }

export function ScheduleActionModal({ cards, action, onClose, onComplete, showToast }: ScheduleActionModalProps) {
  const [dueChoice, setDueChoice] = useState("tomorrow");
  const [customDate, setCustomDate] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const savingRef = useRef(false);
  const pendingMutationRef = useRef<PendingScheduleMutation | null>(null);

  async function submit(): Promise<void> {
    if (savingRef.current) return;
    const rescheduleIntent = action === "reschedule" ? resolveRescheduleIntent(dueChoice, customDate) : undefined;
    if (action === "reschedule" && !rescheduleIntent) {
      pendingMutationRef.current = null;
      setErrorMessage("Choose a valid custom due date.");
      return;
    }
    const cardIds = [...new Set(cards.map((card) => card.cardId))].sort();
    const key = JSON.stringify({ action, cardIds, rescheduleIntent });
    const mutationId = mutationIdForRequest(key);
    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);
    let result;
    try {
      result = await applyScheduleAction({ mutationId, cardIds, action, rescheduleIntent: rescheduleIntent ?? undefined });
      pendingMutationRef.current = null;
    } catch (error: unknown) {
      if (getErrorCode(error) !== null) {
        pendingMutationRef.current = null;
        setErrorMessage(getErrorMessage(error));
      } else {
        setErrorMessage(`${getErrorMessage(error)} Choose the same action again to safely retry the complete selection.`);
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
      showToast("info", `The schedule changes were saved, but the current view could not refresh: ${getErrorMessage(error)}`);
    } finally {
      savingRef.current = false;
      setIsSaving(false);
      close();
    }
  }

  function mutationIdForRequest(key: string): string {
    if (pendingMutationRef.current?.key === key) return pendingMutationRef.current.mutationId;
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
    <Dialog title={action === "reset" ? "Reset schedule" : "Reschedule cards"}
      description={`${cards.length} ${cards.length === 1 ? "card" : "cards"} selected`}
      width="medium" busy={isSaving} dismissible={!isSaving} onClose={() => close()}
      actions={<>
        <Button variant="secondary" disabled={isSaving} onClick={close}>Cancel</Button>
        <Button variant={action === "reset" ? "danger" : "primary"} disabled={isSaving} onClick={() => void submit()}>
          {isSaving ? "Applying…" : action === "reset" ? "Reset schedule" : "Reschedule"}
        </Button>
      </>}>
      {action === "reset" ? (
        <InlineNotice kind="warning">
          The selected cards will become New and ready now. Existing review history remains
          available for analytics. The complete selection changes in one transaction or not at all.
        </InlineNotice>
      ) : (
        <div className={styles.scheduleActionOptions}>
          <label><span>New schedule date</span>
            <NativeSelect value={dueChoice} disabled={isSaving}
              onChange={(event) => { pendingMutationRef.current = null; setDueChoice(event.currentTarget.value); }}>
              <option value="today">Ready now</option>
              <option value="tomorrow">Tomorrow</option>
              <option value="3_days">In 3 local calendar days</option>
              <option value="1_week">In 7 local calendar days</option>
              <option value="custom">Custom local date</option>
            </NativeSelect>
          </label>
          {dueChoice === "custom" ? (
            <label><span>Custom local date</span>
              <TextField type="date" value={customDate} disabled={isSaving}
                onChange={(event) => { pendingMutationRef.current = null; setCustomDate(event.currentTarget.value); }} />
            </label>
          ) : null}
          <p className={styles.mutedCopy}>
            A later date becomes due when that day starts, at the “New day starts at”
            time in Settings. Rescheduling does not record a successful review or change FSRS
            difficulty and stability.
          </p>
        </div>
      )}
      {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
    </Dialog>
  );
}

function resolveRescheduleIntent(choice: string, customDate: string): RescheduleIntent | null {
  if (choice === "today") return { kind: "due_now" };
  if (choice === "tomorrow") return { kind: "tomorrow" };
  if (choice === "3_days") return { kind: "calendar_days", days: 3 };
  if (choice === "1_week") return { kind: "calendar_days", days: 7 };
  if (choice === "custom" && /^\d{4}-\d{2}-\d{2}$/u.test(customDate)) return { kind: "calendar_date", date: customDate };
  return null;
}

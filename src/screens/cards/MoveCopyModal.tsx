import { useMemo, useState, type ChangeEvent } from "react";
import { Modal } from "../../components/Modal";
import { transferCards, type TransferCardsMode } from "../../ipc/cards";
import { getErrorMessage } from "../../ipc/common";
import type { DeckSummary } from "../../ipc/decks";
import { Button } from "../../ui/primitives/Button";
import { Checkbox, RadioGroup } from "../../ui/primitives/FormControls";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { EmptyState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { DialogActions } from "../../ui/overlays/Dialog";
import styles from "./browserPresentation";

export interface TransferableCard { noteId: string; deckId: string; }
interface MoveCopyModalProps {
  cards: TransferableCard[];
  decks: DeckSummary[];
  onClose: () => void;
  onComplete: () => void | Promise<void>;
  showToast?: (kind: "success" | "error" | "info", text: string) => void;
}

export function MoveCopyModal({ cards, decks, onClose, onComplete, showToast }: MoveCopyModalProps) {
  const sourceDeckIds = useMemo(() => new Set(cards.map((card) => card.deckId)), [cards]);
  const destinations = decks.filter((deck) => deck.archivedAt === null && !sourceDeckIds.has(deck.id));
  const [targetDeckId, setTargetDeckId] = useState(destinations[0]?.id ?? "");
  const [mode, setMode] = useState<TransferCardsMode>("move");
  const [skipDuplicates, setSkipDuplicates] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  async function submit(): Promise<void> {
    if (targetDeckId === "" || cards.length === 0 || isSaving) return;
    setIsSaving(true);
    setErrorMessage(null);
    let affectedCount: number;
    let skippedDuplicates: number;
    try {
      const result = await transferCards({
        noteIds: cards.map((card) => card.noteId), targetDeckId, mode, skipDuplicates,
      });
      affectedCount = result.affectedCount;
      skippedDuplicates = result.skippedDuplicates;
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
      setIsSaving(false);
      return;
    }
    const verb = mode === "move" ? "Moved" : "Copied";
    const duplicateCopy = skippedDuplicates > 0
      ? ` ${skippedDuplicates} ${skippedDuplicates === 1 ? "duplicate was" : "duplicates were"} skipped.`
      : "";
    showToast?.("success", `${verb} ${affectedCount} ${affectedCount === 1 ? "card" : "cards"}.${duplicateCopy}`);
    try {
      await onComplete();
    } catch (error: unknown) {
      const message = `The cards were ${mode === "move" ? "moved" : "copied"}, but the current view could not refresh: ${getErrorMessage(error)}`;
      if (showToast) showToast("info", message);
      else setErrorMessage(message);
    } finally {
      setIsSaving(false);
      onClose();
    }
  }

  return (
    <Modal title="Move or copy cards"
      description={`${cards.length} ${cards.length === 1 ? "card is" : "cards are"} selected.`}
      width="medium" busy={isSaving} dismissible={!isSaving}
      onClose={() => { if (!isSaving) onClose(); }}>
      {destinations.length === 0 ? (
        <EmptyState compact title="Create or unarchive another deck first"
          description="A destination deck must be active and different from every selected card’s deck."
          actions={<Button variant="primary" onClick={onClose}>Close</Button>} />
      ) : (
        <div className={styles.screen} aria-busy={isSaving}>
          <RadioGroup label="Action" name="transfer-mode" value={mode} onValueChange={setMode} disabled={isSaving}
            options={[
              { value: "move", ariaLabel: "Move", label: <span><strong>Move</strong><span className="tw:block tw:mt-1 tw:text-sm tw:text-muted-foreground">Keep all review history and reattribute it to the destination deck.</span></span> },
              { value: "copy", ariaLabel: "Copy", label: <span><strong>Copy</strong><span className="tw:block tw:mt-1 tw:text-sm tw:text-muted-foreground">Create new cards with fresh schedules in the destination.</span></span> },
            ]} />
          <label className={styles.moveCopyDestination}>
            <span>Destination deck</span>
            <NativeSelect value={targetDeckId} disabled={isSaving}
              onChange={(event: ChangeEvent<HTMLSelectElement>) => setTargetDeckId(event.currentTarget.value)}>
              {destinations.map((deck) => <option key={deck.id} value={deck.id}>{deck.name}</option>)}
            </NativeSelect>
          </label>
          <label className={styles.compactCheckbox}>
            <Checkbox checked={skipDuplicates} disabled={isSaving} onCheckedChange={setSkipDuplicates} />
            <span>Skip exact duplicates already in the destination</span>
          </label>
          {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
          <DialogActions>
            <Button variant="secondary" disabled={isSaving} onClick={onClose}>Cancel</Button>
            <Button variant="primary" disabled={isSaving || targetDeckId === ""} onClick={() => void submit()}>
              {isSaving ? (mode === "move" ? "Moving…" : "Copying…")
                : `${mode === "move" ? "Move" : "Copy"} ${cards.length} ${cards.length === 1 ? "card" : "cards"}`}
            </Button>
          </DialogActions>
        </div>
      )}
    </Modal>
  );
}

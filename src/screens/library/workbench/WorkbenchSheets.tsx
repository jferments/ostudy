import { useState } from "react";

import { Sheet } from "../../../ui/overlays/Dialog";
import { Button } from "../../../ui/primitives/Button";
import { RadioGroup } from "../../../ui/primitives/FormControls";
import { FormField } from "../../../ui/primitives/FormField";
import { TextField } from "../../../ui/primitives/TextField";
import type { Subject } from "../../../ipc/subjects";

interface NameSheetProps {
  title: string;
  description?: string;
  label: string;
  initialName?: string;
  submitLabel: string;
  maxLength: number;
  onClose: () => void;
  /** Resolves true when the name was saved; the sheet then closes. */
  onSubmit: (name: string) => Promise<boolean>;
}

/** Names or renames one thing: a subject or a deck. */
export function NameSheet({ title, description, label, initialName = "", submitLabel, maxLength, onClose, onSubmit }: NameSheetProps) {
  const [name, setName] = useState(initialName);
  const [busy, setBusy] = useState(false);
  const trimmed = name.trim();
  const unchanged = trimmed === initialName.trim();

  async function submit(): Promise<void> {
    if (trimmed === "" || unchanged || busy) return;
    setBusy(true);
    const saved = await onSubmit(trimmed);
    if (saved) onClose();
    else setBusy(false);
  }

  return (
    <Sheet
      title={title}
      description={description}
      width="small"
      busy={busy}
      dismissible={!busy}
      onClose={onClose}
      onSubmit={() => void submit()}
      actions={
        <>
          <Button disabled={busy} onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" loading={busy} disabled={trimmed === "" || unchanged}>
            {submitLabel}
          </Button>
        </>
      }
    >
      <FormField label={label}>
        <TextField
          data-autofocus="true"
          value={name}
          maxLength={maxLength}
          autoComplete="off"
          onChange={(event) => setName(event.currentTarget.value)}
        />
      </FormField>
    </Sheet>
  );
}

interface MoveToSubjectSheetProps {
  /** What moves: "Lecture 7" or "3 decks". */
  itemLabel: string;
  subjects: readonly Subject[];
  currentSubjectId: string | null | undefined;
  onClose: () => void;
  onMove: (subjectId: string | null) => Promise<boolean>;
}

const NO_SUBJECT = "__none__";

/** Files decks or a document under one subject, or under No subject. */
export function MoveToSubjectSheet({ itemLabel, subjects, currentSubjectId, onClose, onMove }: MoveToSubjectSheetProps) {
  const ordered = [...subjects].sort((left, right) => left.sortOrder - right.sortOrder);
  const initial = currentSubjectId === undefined
    ? (ordered[0]?.id ?? NO_SUBJECT)
    : (currentSubjectId ?? NO_SUBJECT);
  const [choice, setChoice] = useState(initial);
  const [busy, setBusy] = useState(false);

  async function move(): Promise<void> {
    if (busy) return;
    setBusy(true);
    const moved = await onMove(choice === NO_SUBJECT ? null : choice);
    if (moved) onClose();
    else setBusy(false);
  }

  return (
    <Sheet
      title={`Move ${itemLabel}`}
      description="Choose the subject it belongs to. Cards keep their schedules."
      width="small"
      busy={busy}
      dismissible={!busy}
      onClose={onClose}
      onSubmit={() => void move()}
      actions={
        <>
          <Button disabled={busy} onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" loading={busy} disabled={choice === (currentSubjectId ?? NO_SUBJECT) && currentSubjectId !== undefined}>
            Move
          </Button>
        </>
      }
    >
      <RadioGroup
        label="Subject"
        value={choice}
        onValueChange={setChoice}
        options={[
          ...ordered.map((subject) => ({ value: subject.id, label: subject.name })),
          { value: NO_SUBJECT, label: "No subject" },
        ]}
      />
    </Sheet>
  );
}

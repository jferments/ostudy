import { useState } from "react";

import { getErrorMessage } from "../../ipc/common";
import { createSubject, getSubjectLibrary, type SubjectLibrary } from "../../ipc/subjects";
import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { FormField } from "../../ui/primitives/FormField";
import { TextField } from "../../ui/primitives/TextField";

/** The longest subject name native code accepts. */
const MAX_SUBJECT_NAME_CHARS = 120;

interface NewSubjectSheetProps {
  onClose: () => void;
  /** Receives the library the creation answered with. */
  onCreated: (library: SubjectLibrary, name: string) => void;
}

/** Names a new subject. Native code trims the name and orders it last. */
export function NewSubjectSheet({ onClose, onCreated }: NewSubjectSheetProps) {
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const trimmed = name.trim();

  async function create(): Promise<void> {
    if (trimmed === "" || busy) return;
    setBusy(true);
    setError(null);
    try {
      // The library revision guards the subject list; read the current one so
      // a change made elsewhere since Home loaded does not refuse the name.
      const current = await getSubjectLibrary();
      const library = await createSubject({
        name: trimmed,
        expectedLibraryRevision: current.libraryRevision,
      });
      onCreated(library, trimmed);
    } catch (caught: unknown) {
      setError(getErrorMessage(caught));
      setBusy(false);
    }
  }

  return (
    <Sheet
      title="New subject"
      description="A subject holds a course's documents, guides and decks."
      width="small"
      onClose={onClose}
      busy={busy}
      dismissible={!busy}
      onSubmit={() => void create()}
      actions={
        <>
          <Button disabled={busy} onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" loading={busy} disabled={trimmed === ""}>
            Create subject
          </Button>
        </>
      }
    >
      <FormField label="Name" error={error}>
        <TextField
          data-autofocus="true"
          value={name}
          maxLength={MAX_SUBJECT_NAME_CHARS}
          placeholder="For example, Cardiac physiology"
          autoComplete="off"
          onChange={(event) => setName(event.currentTarget.value)}
        />
      </FormField>
    </Sheet>
  );
}

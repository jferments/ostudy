import { useState, type FormEvent } from "react";

import { Sheet } from "../../../ui/overlays/Dialog";
import { Button } from "../../../ui/primitives/Button";
import styles from "./DeckLibrarySheets.module.css";
import type { LibrarySheetState } from "./types";

interface DeckLibrarySheetsProps {
  sheet: LibrarySheetState | null;
  busy: boolean;
  onClose(): void;
  onCreateDeck(name: string, description: string): Promise<void>;
  onSaveFolder(name: string): Promise<void>;
}

export function DeckLibrarySheets({
  sheet,
  busy,
  onClose,
  onCreateDeck,
  onSaveFolder,
}: DeckLibrarySheetsProps) {
  if (!sheet) return null;
  if (sheet.kind === "create-deck") {
    return <DeckCreateSheet busy={busy} onClose={onClose} onCreate={onCreateDeck} />;
  }
  return (
    <FolderSheet
      key={`${sheet.kind}:${sheet.kind === "rename-folder" ? sheet.folder.id : "new"}`}
      sheet={sheet}
      busy={busy}
      onClose={onClose}
      onSave={onSaveFolder}
    />
  );
}

function DeckCreateSheet({
  busy,
  onClose,
  onCreate,
}: {
  busy: boolean;
  onClose(): void;
  onCreate(name: string, description: string): Promise<void>;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  return (
    <Sheet
      title="Create a deck"
      description="Add a new place for related flashcards."
      width="small"
      onClose={onClose}
      busy={busy}
      dismissible={!busy}
    >
      <form
        className={styles.form}
        onSubmit={(event) => {
          event.preventDefault();
          void onCreate(name, description);
        }}
      >
        <label>
          <span>Name</span>
          <input
            data-autofocus="true"
            value={name}
            maxLength={200}
            onChange={(event) => setName(event.currentTarget.value)}
          />
        </label>
        <label>
          <span>
            Description <small>optional</small>
          </span>
          <textarea
            value={description}
            maxLength={2000}
            rows={4}
            onChange={(event) => setDescription(event.currentTarget.value)}
          />
        </label>
        <div className={styles.actions}>
          <Button disabled={busy} onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" loading={busy} disabled={name.trim() === ""}>
            Create deck
          </Button>
        </div>
      </form>
    </Sheet>
  );
}

function FolderSheet({
  sheet,
  busy,
  onClose,
  onSave,
}: {
  sheet: Exclude<LibrarySheetState, { kind: "create-deck" }>;
  busy: boolean;
  onClose(): void;
  onSave(name: string): Promise<void>;
}) {
  const [name, setName] = useState(sheet.name);
  return (
    <Sheet
      title={sheet.kind === "create-folder" ? "Create a folder" : "Rename folder"}
      width="small"
      onClose={onClose}
      busy={busy}
      dismissible={!busy}
    >
      <form
        className={styles.form}
        onSubmit={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          void onSave(name);
        }}
      >
        <label>
          <span>Folder name</span>
          <input
            data-autofocus="true"
            value={name}
            maxLength={120}
            onChange={(event) => setName(event.currentTarget.value)}
          />
        </label>
        <div className={styles.actions}>
          <Button disabled={busy} onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" loading={busy} disabled={name.trim() === ""}>
            {sheet.kind === "create-folder" ? "Create folder" : "Save name"}
          </Button>
        </div>
      </form>
    </Sheet>
  );
}

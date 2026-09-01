import { useState, type FormEvent } from "react";

import { Sheet } from "../../../ui/overlays/Dialog";
import { Button } from "../../../ui/primitives/Button";
import type { DeckFolder } from "../../../ipc/deckFolders";
import type { DeckSummary } from "../../../ipc/decks";
import type { SelectedPracticeRequest } from "../../../ipc/practice";
import styles from "./DeckLibrarySheets.module.css";
import { PracticeSetupSheet } from "./PracticeSetupSheet";
import type { LibrarySheetState } from "./types";

interface DeckLibrarySheetsProps {
  sheet: LibrarySheetState | null;
  busy: boolean;
  folders: readonly DeckFolder[];
  decks: readonly DeckSummary[];
  practiceShuffle: boolean;
  onClose(): void;
  onCreateDeck(name: string, description: string): Promise<void>;
  onSaveFolder(name: string): Promise<void>;
  onMoveSelection(targetFolderId: string | null): Promise<void>;
  onStartPractice(request: SelectedPracticeRequest): void | Promise<void>;
}

export function DeckLibrarySheets({
  sheet,
  busy,
  folders,
  decks,
  practiceShuffle,
  onClose,
  onCreateDeck,
  onSaveFolder,
  onMoveSelection,
  onStartPractice,
}: DeckLibrarySheetsProps) {
  if (!sheet) return null;
  if (sheet.kind === "create-deck") {
    return <DeckCreateSheet busy={busy} onClose={onClose} onCreate={onCreateDeck} />;
  }
  if (sheet.kind === "move-selection") {
    return (
      <MoveSelectionSheet
        folders={folders}
        busy={busy}
        onClose={onClose}
        onMove={onMoveSelection}
      />
    );
  }
  if (sheet.kind === "practice-setup") {
    return (
      <PracticeSetupSheet
        scope={sheet.scope}
        currentDecks={decks}
        initialShuffle={practiceShuffle}
        onClose={onClose}
        onStart={onStartPractice}
      />
    );
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

function MoveSelectionSheet({
  folders,
  busy,
  onClose,
  onMove,
}: {
  folders: readonly DeckFolder[];
  busy: boolean;
  onClose(): void;
  onMove(targetFolderId: string | null): Promise<void>;
}) {
  const [targetFolderId, setTargetFolderId] = useState("");
  return (
    <Sheet
      title="Move selected decks"
      description="We’ll move the selected decks one at a time in the order shown. If one deck can’t be moved, decks already moved stay in their new location and we stop before moving the rest."
      width="small"
      onClose={onClose}
      busy={busy}
      dismissible={!busy}
    >
      <form
        className={styles.form}
        onSubmit={(event) => {
          event.preventDefault();
          void onMove(targetFolderId === "" ? null : targetFolderId);
        }}
      >
        <label>
          <span>Destination</span>
          <select
            data-autofocus="true"
            value={targetFolderId}
            disabled={busy}
            onChange={(event) => setTargetFolderId(event.currentTarget.value)}
          >
            <option value="">Unsorted decks</option>
            {folders.map((folder) => (
              <option key={folder.id} value={folder.id}>
                {folder.name}
              </option>
            ))}
          </select>
        </label>
        <div className={styles.actions}>
          <Button disabled={busy} onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" loading={busy}>
            Move selected
          </Button>
        </div>
      </form>
    </Sheet>
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
  sheet: Exclude<
    LibrarySheetState,
    { kind: "create-deck" } | { kind: "move-selection" } | { kind: "practice-setup" }
  >;
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

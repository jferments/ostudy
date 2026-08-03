import { useState } from "react";

import { GeneratedImportDialog } from "../components/GeneratedImportDialog";
import type { FlashcardImportResult } from "../generatedImport";
import type { AppSettings, Deck } from "../ipc";
import { DecksScreen } from "./DecksScreen";

interface DecksImportScreenProps {
  decks: Deck[];
  settings: AppSettings;
  refreshDecks: () => Promise<Deck[]>;
  onPracticeDeck: (deckId: string) => void;
  onDirtyChange: (dirty: boolean) => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

export function DecksImportScreen(props: DecksImportScreenProps) {
  const [showGeneratedImport, setShowGeneratedImport] = useState(false);

  async function finishImport(result: FlashcardImportResult): Promise<void> {
    await props.refreshDecks();
    setShowGeneratedImport(false);
    const skipped = result.skippedDuplicateCount > 0
      ? ` ${result.skippedDuplicateCount} exact ${result.skippedDuplicateCount === 1 ? "duplicate was" : "duplicates were"} skipped.`
      : "";
    props.showToast(
      result.createdNoteCount > 0 ? "success" : "info",
      `Imported ${result.createdNoteCount} source ${result.createdNoteCount === 1 ? "note" : "notes"} as ${result.createdCardCount} scheduled ${result.createdCardCount === 1 ? "card" : "cards"}.${skipped}`,
    );
  }

  return (
    <div className="decks-import-boundary">
      <div className="decks-import-toolbar" aria-label="Generated flashcard import">
        <button
          type="button"
          className="secondary-button"
          onClick={() => setShowGeneratedImport(true)}
        >
          Import generated JSON
        </button>
      </div>
      <DecksScreen {...props} />
      {showGeneratedImport ? (
        <GeneratedImportDialog
          decks={props.decks}
          onClose={() => setShowGeneratedImport(false)}
          onImported={finishImport}
        />
      ) : null}
    </div>
  );
}

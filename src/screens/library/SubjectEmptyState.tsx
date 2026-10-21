import { useRef, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { DecksController } from "../../controllers/decksController";
import { useDocumentJobs } from "../../controllers/DocumentJobsProvider";
import { useOptionalHomeSummary } from "../../controllers/HomeSummaryProvider";
import { getErrorMessage } from "../../ipc/common";
import type { DeckSummary } from "../../ipc/decks";
import { getSubjectLibrary, moveDeckToSubject, moveDocumentToSubject } from "../../ipc/subjects";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { EmptyState } from "../../ui/primitives/EmptyState";
import { DeckTransferDialog } from "../flashcards/DeckTransferDialog";
import { NameSheet } from "./workbench/WorkbenchSheets";

const MAX_DECK_NAME = 200;

interface SubjectEmptyStateProps {
  subjectId: string;
  name: string;
  subjects: { id: string; name: string }[];
  refreshDecks: () => Promise<DeckSummary[]>;
  showToast: ShowToast;
  onOpenDeck: (deckId: string) => void;
  /** Documents, where an uploaded document's processing shows. */
  onShowDocuments: () => void;
}

/**
 * A subject with nothing in it: its three ways to start, each filing what it
 * makes in the subject. Filing follows the committed upload, create or import,
 * so a refused move leaves the new material under No subject.
 */
export function SubjectEmptyState({ subjectId, name, subjects, refreshDecks, showToast, onOpenDeck, onShowDocuments }: SubjectEmptyStateProps) {
  const jobs = useDocumentJobs();
  const acceptLibrary = useOptionalHomeSummary()?.acceptLibrary;
  const decksRef = useRef<DecksController | null>(null);
  if (decksRef.current === null) decksRef.current = new DecksController();
  const [sheet, setSheet] = useState<"new-deck" | "import" | null>(null);

  function upload(): void {
    if (!jobs) return;
    void jobs.uploadDocument(undefined, async (documentId) => {
      try {
        const current = await getSubjectLibrary();
        const next = await moveDocumentToSubject({ documentId, subjectId, expectedDocumentFilingRevision: current.documentFilingRevision });
        acceptLibrary?.(next);
      } finally {
        onShowDocuments();
      }
    });
  }

  async function createDeck(deckName: string): Promise<boolean> {
    let deck: DeckSummary;
    try {
      deck = await decksRef.current!.create({ name: deckName });
    } catch (caught: unknown) {
      showToast("error", getErrorMessage(caught));
      return false;
    }
    try {
      const current = await getSubjectLibrary();
      const next = await moveDeckToSubject({ deckId: deck.id, subjectId, expectedLibraryRevision: current.libraryRevision });
      acceptLibrary?.(next);
      showToast("success", `Created “${deck.name}”.`);
    } catch (caught: unknown) {
      showToast("info", `Created “${deck.name}”. It stays under No subject: ${getErrorMessage(caught)}`);
    }
    await refreshDecks().catch(() => undefined);
    onOpenDeck(deck.id);
    return true;
  }

  return (
    <>
      <EmptyState
        large
        icon={<Icon name="library" size={28} />}
        title={`Nothing in ${name} yet`}
        actions={
          <div className="tw:flex tw:flex-wrap tw:justify-center tw:gap-2">
            {jobs ? (
              <Button variant="primary" disabled={jobs.isUploading} onClick={upload}>
                <Icon name="upload" size={16} />Upload a document
              </Button>
            ) : null}
            <Button onClick={() => setSheet("new-deck")}><Icon name="plus" size={16} />New deck</Button>
            <Button onClick={() => setSheet("import")}><Icon name="import" size={16} />Import deck</Button>
          </div>
        }
      />
      {sheet === "new-deck" ? (
        <NameSheet title="New deck" label="Name" submitLabel="Create deck" maxLength={MAX_DECK_NAME}
          onClose={() => setSheet(null)} onSubmit={createDeck} />
      ) : null}
      {sheet === "import" ? (
        <DeckTransferDialog
          request={{ kind: "import", subjects, subjectId }}
          refreshDecks={refreshDecks}
          onImportedDeck={(outcome) => onOpenDeck(outcome.deckId)}
          onClose={() => setSheet(null)}
          showToast={showToast}
        />
      ) : null}
    </>
  );
}

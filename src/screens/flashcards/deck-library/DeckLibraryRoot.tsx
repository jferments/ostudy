import { Icon } from "../../../ui/icons/Icon";
import { Button } from "../../../ui/primitives/Button";
import { LoadingState } from "../../../ui/primitives/EmptyState";
import { InlineNotice } from "../../../ui/primitives/InlineNotice";
import { VisuallyHidden } from "../../../ui/primitives/VisuallyHidden";
import { DeckLibrarySheets } from "./DeckLibrarySheets";
import { DeckLibraryView } from "./DeckLibraryView";
import styles from "./DeckLibraryView.module.css";
import type { DeckLibrarySnapshot } from "./organizationModel";
import type { DeckLibraryController } from "./useDeckLibraryController";
import { useDeckLibraryReorder } from "./useDeckLibraryReorder";

interface DeckLibraryRootProps {
  controller: DeckLibraryController;
  onPracticeDeck(deckId: string): void | Promise<void>;
}

export function DeckLibraryRoot({ controller, onPracticeDeck }: DeckLibraryRootProps) {
  return (
    <div className={styles.root} aria-busy={controller.isLoading || controller.isMutating}>
      <div className={styles.toolbar} aria-label="Deck library controls">
        <div className={styles.toolbarActions}>
          <Button variant="primary" onClick={controller.openDeckCreator}>
            <Icon name="plus" size={18} /> New deck
          </Button>
        </div>
      </div>

      <VisuallyHidden as="div" aria-live="polite">
        {controller.announcement}
      </VisuallyHidden>
      {controller.errorMessage ? (
        <InlineNotice kind="error">{controller.errorMessage}</InlineNotice>
      ) : null}
      {controller.isLoading ? <LoadingState label="Loading deck organization…" /> : null}
      {controller.snapshot ? (
        <ReadyDeckLibrary
          controller={controller}
          snapshot={controller.snapshot}
          onPracticeDeck={(deckId) => void onPracticeDeck(deckId)}
        />
      ) : null}
      <DeckLibrarySheets
        sheet={controller.sheet}
        busy={controller.isMutating}
        onClose={controller.closeSheet}
        onCreateDeck={controller.createDeck}
        onSaveFolder={controller.saveFolder}
      />
    </div>
  );
}

function ReadyDeckLibrary({
  controller,
  snapshot,
  onPracticeDeck,
}: DeckLibraryRootProps & { snapshot: DeckLibrarySnapshot }) {
  const reorder = useDeckLibraryReorder(snapshot, controller.isMutating, controller.move);
  return (
    <DeckLibraryView
      controller={controller}
      reorder={reorder}
      onPracticeDeck={onPracticeDeck}
    />
  );
}

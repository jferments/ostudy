import { useRef, useState } from "react";

import { AndroidDeckSharePanel } from "../../components/AndroidDeckSharePanel";
import type { ShowToast } from "../../components/Toast";
import { DecksController } from "../../controllers/decksController";
import { createOperationId, getErrorMessage } from "../../ipc/common";
import type { DeckSummary } from "../../ipc/decks";
import { publishExport, selectExternalImport, shareExport } from "../../platform/files";
import { Icon } from "../../ui/icons/Icon";
import { ResponsiveGrid, Stack } from "../../ui/layout/Layout";
import { Button } from "../../ui/primitives/Button";
import { Surface } from "../../ui/primitives/Surface";
import styles from "./FlashcardsWorkspace.module.css";

export function TransferWorkspace({
  decks,
  refreshDecks,
  showToast,
}: {
  decks: DeckSummary[];
  refreshDecks: () => Promise<DeckSummary[]>;
  showToast: ShowToast;
}) {
  const activeDecks = decks.filter((deck) => deck.archivedAt === null);
  const [selectedDeckId, setSelectedDeckId] = useState(activeDecks[0]?.id ?? "");
  const [busy, setBusy] = useState<"import" | "export" | "share" | null>(null);
  const controllerRef = useRef<DecksController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new DecksController();
  const controller = controllerRef.current;

  async function importContent(): Promise<void> {
    if (busy) return;
    setBusy("import");
    try {
      const selected = await selectExternalImport("study_content");
      if (!selected) return;
      if (selected.detectedKind === "studydeck") {
        const result = await controller.importStudydeck(selected.id, createOperationId());
        await refreshDecks();
        showToast("success", `Imported “${result.deck.name}” with ${result.cardCount} ${result.cardCount === 1 ? "card" : "cards"}.`);
      } else if (selected.detectedKind === "generated_json") {
        const result = await controller.importGenerated(selected.id);
        await refreshDecks();
        showToast("success", `Imported ${result.createdCardCount} scheduled ${result.createdCardCount === 1 ? "card" : "cards"}.`);
      } else {
        throw new Error("Select an ostudy deck or generated-card file.");
      }
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      setBusy(null);
    }
  }

  async function transfer(kind: "export" | "share"): Promise<void> {
    if (busy || !selectedDeckId) return;
    setBusy(kind);
    try {
      const prepared = await controller.prepareExport(selectedDeckId, createOperationId());
      if (kind === "share") {
        await shareExport(prepared.preparedExport.id);
        showToast("success", "Opened the system share interface.");
      } else {
        const receipt = await publishExport(prepared.preparedExport.id);
        if (receipt) showToast("success", `Exported ${receipt.displayName}.`);
      }
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      setBusy(null);
    }
  }

  return (
    <Stack gap="var(--ostudy-space-5)">
      <ResponsiveGrid minItemWidth="280px">
        <Surface level={2} className={styles.taskCard}>
          <span className={styles.taskIcon}><Icon name="import" size={24} /></span>
          <h2>Import flashcards</h2>
          <p>Import an ostudy deck or generated-card JSON file into the local library.</p>
          <Button variant="primary" loading={busy === "import"} onClick={() => void importContent()}>Import deck or cards</Button>
        </Surface>

        <Surface level={2} className={styles.taskCard}>
          <span className={styles.taskIcon}><Icon name="export" size={24} /></span>
          <h2>Export or share a deck</h2>
          {activeDecks.length > 0 ? (
            <>
              <label className={styles.deckSelect}>
                <span>Deck</span>
                <select value={selectedDeckId} onChange={(event) => setSelectedDeckId(event.currentTarget.value)}>
                  {activeDecks.map((deck) => <option key={deck.id} value={deck.id}>{deck.name}</option>)}
                </select>
              </label>
              <div className={styles.taskActions}>
                <Button loading={busy === "export"} onClick={() => void transfer("export")}>Export file</Button>
                <Button variant="primary" loading={busy === "share"} onClick={() => void transfer("share")}>Share</Button>
              </div>
            </>
          ) : (
            <p>Create a deck before exporting or sharing study content.</p>
          )}
        </Surface>
      </ResponsiveGrid>
      <AndroidDeckSharePanel decks={decks} showToast={showToast} />
    </Stack>
  );
}

import { useEffect, useRef, useState } from "react";
import type { ShowToast } from "../../components/Toast";
import { DeckTransferController, type DeckImportOutcome } from "../../controllers/deckTransferController";
import { getErrorMessage } from "../../ipc/common";
import type { DeckSummary } from "../../ipc/decks";
import type { OperationProgress } from "../../ipc/operationProgress";
import { usePlatformCapabilities } from "../../platform/PlatformProvider";
import { Icon } from "../../ui/icons/Icon";
import { Dialog } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { Badge } from "../../ui/primitives/Badge";
import { Panel } from "../../ui/primitives/Panel";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { Progress } from "../../ui/primitives/Progress";
import { AnkiImportReportView } from "./AnkiImportReport";

export type DeckTransferDialogRequest =
  | { kind: "import" }
  | { kind: "export"; decks: DeckSummary[] };
interface DeckTransferDialogProps {
  request: DeckTransferDialogRequest;
  refreshDecks(): Promise<DeckSummary[]>;
  onImportedDeck?(outcome: DeckImportOutcome): void | Promise<void>;
  onClose(): void;
  showToast: ShowToast;
}

export function DeckTransferDialog({ request, refreshDecks, onImportedDeck, onClose, showToast }: DeckTransferDialogProps) {
  const controllerRef = useRef<DeckTransferController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new DeckTransferController();
  const controller = controllerRef.current;
  const capabilities = usePlatformCapabilities();
  const [busy, setBusy] = useState<"import" | "export" | "share" | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [importedOutcome, setImportedOutcome] = useState<DeckImportOutcome | null>(null);
  const [importProgress, setImportProgress] = useState<OperationProgress | null>(null);
  const exportDecks = request.kind === "export" ? request.decks : [];
  const canShare = exportDecks.length === 1 && capabilities.native?.services.systemShare.state === "available";
  useEffect(() => () => controller.invalidate(), [controller]);

  async function openImportedDeck(outcome: DeckImportOutcome): Promise<void> {
    onClose();
    try {
      await onImportedDeck?.(outcome);
    } catch (error: unknown) {
      // Navigation follows the durable import boundary; it cannot make that import fail.
      showToast("info", `The deck was imported but could not be opened: ${getErrorMessage(error)}`);
    }
  }
  async function importDeck(): Promise<void> {
    if (busy || importedOutcome) return;
    setBusy("import");
    setErrorMessage(null);
    setImportProgress(null);
    try {
      const outcome = await controller.importFromFile(setImportProgress);
      if (!outcome) { setImportProgress(null); return; }
      let refreshError: string | null = null;
      try { await refreshDecks(); } catch (error: unknown) { refreshError = getErrorMessage(error); }
      const message = refreshError ? `${outcome.message} The library could not refresh immediately: ${refreshError}` : outcome.message;
      showToast(refreshError ? "info" : "success", message);
      // A post-commit report, never an import preview or a request to repeat the write.
      if (outcome.ankiReport) setImportedOutcome({ ...outcome, message });
      else await openImportedDeck(outcome);
    } catch (error: unknown) {
      setImportProgress(null);
      setErrorMessage(getErrorMessage(error));
    } finally { setBusy(null); }
  }
  async function saveDecks(): Promise<void> {
    if (busy || exportDecks.length === 0) return;
    setBusy("export");
    setErrorMessage(null);
    try {
      const outcome = await controller.exportDecks(exportDecks);
      if (outcome.cancelled) {
        if (outcome.exported.length > 0) showToast("info", `Saved ${outcome.exported.length} of ${outcome.requestedCount} deck files before export was canceled.`);
        onClose();
        return;
      }
      const totalCards = outcome.exported.reduce((sum, exported) => sum + exported.cardCount, 0);
      const onlyExport = outcome.exported.length === 1 ? outcome.exported[0] : undefined;
      showToast("success", onlyExport
        ? `Exported ${onlyExport.receipt.displayName}.`
        : `Exported ${outcome.exported.length} deck files with ${totalCards} total cards.`);
      onClose();
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
    finally { setBusy(null); }
  }
  async function shareDeck(): Promise<void> {
    const deck = exportDecks[0];
    if (!deck || busy) return;
    setBusy("share");
    setErrorMessage(null);
    try {
      const outcome = await controller.shareDeck(deck);
      if (!outcome) return;
      showToast("info", `Opened system sharing for “${deck.name}” with ${outcome.cardCount} ${outcome.cardCount === 1 ? "card" : "cards"}.`);
      onClose();
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
    finally { setBusy(null); }
  }

  const importing = request.kind === "import";
  const activeAnkiProgress = importing && busy === "import" && importProgress?.operationKind === "anki_import" ? importProgress : null;
  const title = importedOutcome ? "Anki import complete" : importing ? "Import deck"
    : exportDecks.length === 1 ? `Export “${exportDecks[0]?.name ?? "deck"}”` : `Export ${exportDecks.length} selected decks`;
  const description = importedOutcome ? "The deck has been saved. Review conversion details before studying."
    : importing ? "Import an ostudy .studydeck file, a compatible Anki .apkg deck, or generated-card JSON."
      : exportDecks.length === 1 ? "Save a portable .studydeck file, or share it through the operating system when available."
        : "Each selected deck is saved as its own portable .studydeck file.";

  return <Dialog title={title} description={description} width="medium" busy={busy !== null} dismissible={busy === null} onClose={onClose}
    actions={<>
      <Button variant="ghost" disabled={busy !== null} onClick={onClose}>{importedOutcome ? "Close" : "Cancel"}</Button>
      {importedOutcome ? <Button variant="primary" onClick={() => void openImportedDeck(importedOutcome)}>{onImportedDeck ? "Open imported deck" : "Done"}</Button>
        : importing ? <Button variant="primary" loading={busy === "import"} onClick={() => void importDeck()}>{busy === "import" ? "Importing…" : "Import deck"}</Button>
          : <>
            {canShare ? <Button variant="secondary" loading={busy === "share"} disabled={busy !== null} onClick={() => void shareDeck()}><Icon name="export" size={18} /> Share</Button> : null}
            <Button variant="primary" loading={busy === "export"} disabled={busy !== null || exportDecks.length === 0} onClick={() => void saveDecks()}><Icon name="save" size={18} />{exportDecks.length === 1 ? "Save file" : "Save deck files"}</Button>
          </>}
    </>}>
    <div className="tw:grid tw:min-w-0 tw:gap-6">
      {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
      {importedOutcome?.ankiReport ? <AnkiImportReportView report={importedOutcome.ankiReport} message={importedOutcome.message} />
        : activeAnkiProgress ? <AnkiProgressView progress={activeAnkiProgress} />
          : importing ? <Panel title="Choose a flashcard file">
            <p className="tw:m-0 tw:text-label tw:leading-relaxed tw:text-muted-foreground">
              Import an ostudy deck, compatible Anki .apkg deck, or validated generated-card JSON file. For newer Anki exports, enable “Support older Anki versions” and “Include media” when exporting from Anki.
            </p>
            <div className="tw:flex tw:flex-wrap tw:gap-2"><Badge>.studydeck</Badge><Badge>Anki .apkg</Badge><Badge>.ostudy.json</Badge></div>
          </Panel> : <Panel title="Decks to export" aria-label="Decks to export">
            <ul className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0">
              {exportDecks.map((deck) => <li key={deck.id} className="tw:flex tw:items-center tw:gap-3 tw:rounded-lg tw:bg-muted tw:p-3">
                <span className="tw:shrink-0 tw:text-accent-foreground" aria-hidden="true"><Icon name="cards" size={20} /></span>
                <span className="tw:grid tw:min-w-0 tw:gap-1"><strong className="tw:break-words tw:font-medium">{deck.name}</strong>
                  <small className="tw:text-muted-foreground">{deck.generatedCardCount} {deck.generatedCardCount === 1 ? "card" : "cards"}</small></span>
              </li>)}
            </ul>
          </Panel>}
    </div>
  </Dialog>;
}

function AnkiProgressView({ progress }: { progress: OperationProgress }) {
  const total = progress.total;
  const processed = progress.processed ?? 0;
  const determinate = total !== null && total > 0;
  const valueText = determinate ? `${processed} of ${total} cards processed; ${progress.succeeded} imported; ${progress.failed} skipped` : phaseLabel(progress.phase);
  return <Panel title={phaseLabel(progress.phase)} aria-label="Anki import progress" actions={<Badge>{determinate ? `${processed} / ${total}` : "Preparing…"}</Badge>}>
    {determinate ? <Progress value={processed} max={total} label="Anki import progress" valueText={valueText} size="large" />
      : <InlineProgress>{valueText}</InlineProgress>}
    <p className="tw:m-0 tw:text-label tw:text-muted-foreground">{determinate ? `${progress.succeeded} imported · ${progress.failed} skipped` : "Reading the package and counting cards…"}</p>
  </Panel>;
}
function phaseLabel(phase: string): string {
  switch (phase) {
    case "reading_package": return "Reading Anki package…";
    case "converting_cards": return "Converting cards…";
    case "importing_media": return "Importing media…";
    case "saving_deck": return "Saving deck…";
    default: return "Importing Anki deck…";
  }
}

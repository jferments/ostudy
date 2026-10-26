import { useEffect, useRef, useState } from "react";
import type { ShowToast } from "../../components/Toast";
import { useOptionalHomeSummary } from "../../controllers/HomeSummaryProvider";
import { DeckTransferController, type DeckImportOutcome } from "../../controllers/deckTransferController";
import {
  detectQuizletSeparators,
  parseQuizletText,
  type QuizletRowSeparator,
  type QuizletSeparators,
  type QuizletTermSeparator,
} from "../../controllers/quizletImport";
import { getErrorMessage } from "../../ipc/common";
import type { DeckSummary } from "../../ipc/decks";
import type { OperationProgress } from "../../ipc/operationProgress";
import { getSubjectLibrary, moveDeckToSubject } from "../../ipc/subjects";
import { usePlatformCapabilities } from "../../platform/PlatformProvider";
import { Icon } from "../../ui/icons/Icon";
import { Dialog } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { Badge } from "../../ui/primitives/Badge";
import { FormField } from "../../ui/primitives/FormField";
import { Textarea } from "../../ui/primitives/FormControls";
import { TextField } from "../../ui/primitives/TextField";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { Panel } from "../../ui/primitives/Panel";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { ItemTrigger } from "../../ui/primitives/Item";
import { Progress } from "../../ui/primitives/Progress";
import { AnkiImportReportView } from "./AnkiImportReport";

export type DeckTransferDialogRequest =
  /** With subjects, the dialog offers Into subject, starting on `subjectId` (null is No subject). */
  | { kind: "import"; subjects?: { id: string; name: string }[]; subjectId?: string | null }
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
  const [pasting, setPasting] = useState(false);
  const [pastedName, setPastedName] = useState("");
  const [pastedText, setPastedText] = useState("");
  const [separators, setSeparators] = useState<QuizletSeparators>({ term: "tab", row: "newline" });
  const pasted = parseQuizletText(pastedText, separators);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [importedOutcome, setImportedOutcome] = useState<DeckImportOutcome | null>(null);
  const [importProgress, setImportProgress] = useState<OperationProgress | null>(null);
  const exportDecks = request.kind === "export" ? request.decks : [];
  const subjects = request.kind === "import" ? request.subjects ?? [] : [];
  const [intoSubjectId, setIntoSubjectId] = useState<string | null>(request.kind === "import" ? request.subjectId ?? null : null);
  const acceptLibrary = useOptionalHomeSummary()?.acceptLibrary;
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
      await fileImportedDeck(outcome);
    } catch (error: unknown) {
      setImportProgress(null);
      setErrorMessage(getErrorMessage(error));
    } finally { setBusy(null); }
  }
  async function importPasted(): Promise<void> {
    if (busy || pasted.cards.length === 0 || pastedName.trim() === "") return;
    setBusy("import");
    setErrorMessage(null);
    try {
      await fileImportedDeck(await controller.importPastedCards(pastedName, pasted.cards));
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally { setBusy(null); }
  }
  async function fileImportedDeck(outcome: DeckImportOutcome): Promise<void> {
    // Filing the deck follows the committed import; a refused move leaves it under No subject.
    let filingError: string | null = null;
    if (intoSubjectId !== null) {
      try {
        const current = await getSubjectLibrary();
        const next = await moveDeckToSubject({ deckId: outcome.deckId, subjectId: intoSubjectId, expectedLibraryRevision: current.libraryRevision });
        acceptLibrary?.(next);
      } catch (error: unknown) { filingError = getErrorMessage(error); }
    }
    let refreshError: string | null = null;
    try { await refreshDecks(); } catch (error: unknown) { refreshError = getErrorMessage(error); }
    const message = [
      outcome.message,
      filingError ? `It stays under No subject: ${filingError}` : null,
      refreshError ? `The library could not refresh immediately: ${refreshError}` : null,
    ].filter(Boolean).join(" ");
    showToast(refreshError || filingError ? "info" : "success", message);
    // A post-commit report, never an import preview or a request to repeat the write.
    if (outcome.ankiReport) setImportedOutcome({ ...outcome, message });
    else await openImportedDeck(outcome);
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
  const title = importedOutcome ? "Anki import complete" : importing && pasting ? "Import from Quizlet" : importing ? "Import deck"
    : exportDecks.length === 1 ? `Export “${exportDecks[0]?.name ?? "deck"}”` : `Export ${exportDecks.length} selected decks`;
  const description = importedOutcome ? "The deck has been saved. Review conversion details before studying."
    : importing && pasting ? "In Quizlet, open the set, choose ⋯ then Export, copy the text and paste it here."
      : importing ? undefined
      : exportDecks.length === 1 ? "Save a portable .studydeck file, or share it through the operating system when available."
        : "Each selected deck is saved as its own portable .studydeck file.";

  return <Dialog title={title} description={description} width="medium" busy={busy !== null} dismissible={busy === null} onClose={onClose}
    actions={<>
      {importing && pasting && !importedOutcome
        ? <Button variant="ghost" disabled={busy !== null} onClick={() => { setPasting(false); setErrorMessage(null); }}>Back</Button>
        : <Button variant="ghost" disabled={busy !== null} onClick={onClose}>{importedOutcome ? "Close" : "Cancel"}</Button>}
      {importedOutcome ? <Button variant="primary" onClick={() => void openImportedDeck(importedOutcome)}>{onImportedDeck ? "Open imported deck" : "Done"}</Button>
        : importing && pasting ? <Button variant="primary" loading={busy === "import"} disabled={busy !== null || pasted.cards.length === 0 || pastedName.trim() === ""} onClick={() => void importPasted()}>
          <Icon name="import" size={18} />Import {pasted.cards.length} {pasted.cards.length === 1 ? "card" : "cards"}
        </Button>
        : importing ? null
          : <>
            {canShare ? <Button variant="secondary" loading={busy === "share"} disabled={busy !== null} onClick={() => void shareDeck()}><Icon name="export" size={18} /> Share</Button> : null}
            <Button variant="primary" loading={busy === "export"} disabled={busy !== null || exportDecks.length === 0} onClick={() => void saveDecks()}><Icon name="save" size={18} />{exportDecks.length === 1 ? "Save file" : "Save deck files"}</Button>
          </>}
    </>}>
    <div className="tw:grid tw:min-w-0 tw:gap-6">
      {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
      {importedOutcome?.ankiReport ? <AnkiImportReportView report={importedOutcome.ankiReport} message={importedOutcome.message} />
        : activeAnkiProgress ? <AnkiProgressView progress={activeAnkiProgress} />
          : importing && pasting ? <>
            <FormField label="Deck name" required>
              <TextField value={pastedName} disabled={busy !== null} placeholder="Spanish verbs" onChange={(event) => setPastedName(event.currentTarget.value)} />
            </FormField>
            <FormField label="Quizlet text" description="Any list with a term and a definition on each line works too.">
              <Textarea rows={8} value={pastedText} disabled={busy !== null} placeholder={"hablar\tto speak\ncomer\tto eat"} data-testid="quizlet-text"
                onChange={(event) => {
                  const text = event.currentTarget.value;
                  if (pastedText.trim() === "") setSeparators(detectQuizletSeparators(text));
                  setPastedText(text);
                }} />
            </FormField>
            <div className="tw:grid tw:gap-3 tw:medium:grid-cols-2">
              <FormField label="Between term and definition">
                <NativeSelect value={separators.term} disabled={busy !== null} onChange={(event) => setSeparators({ ...separators, term: event.currentTarget.value as QuizletTermSeparator })}>
                  <option value="tab">Tab</option>
                  <option value="comma">Comma</option>
                </NativeSelect>
              </FormField>
              <FormField label="Between cards">
                <NativeSelect value={separators.row} disabled={busy !== null} onChange={(event) => setSeparators({ ...separators, row: event.currentTarget.value as QuizletRowSeparator })}>
                  <option value="newline">New line</option>
                  <option value="semicolon">Semicolon</option>
                </NativeSelect>
              </FormField>
            </div>
            {pastedText.trim() !== "" ? (
              <p className="tw:m-0 tw:text-small tw:text-muted-foreground" aria-live="polite">
                {pasted.cards.length} {pasted.cards.length === 1 ? "card" : "cards"} found
                {pasted.skipped > 0 ? `; ${pasted.skipped} ${pasted.skipped === 1 ? "line has" : "lines have"} no definition and will be left out` : ""}.
                {pasted.cards[0] ? ` First: “${pasted.cards[0].question}” → “${pasted.cards[0].answer}”.` : ""}
              </p>
            ) : null}
            {subjects.length > 0 ? (
              <FormField label="Into subject">
                <NativeSelect value={intoSubjectId ?? ""} disabled={busy !== null} onChange={(event) => setIntoSubjectId(event.currentTarget.value || null)}>
                  <option value="">No subject</option>
                  {subjects.map((subject) => <option key={subject.id} value={subject.id}>{subject.name}</option>)}
                </NativeSelect>
              </FormField>
            ) : null}
          </> : importing ? <>
            <ItemTrigger data-testid="deck-import-choose" disabled={busy !== null} onClick={() => void importDeck()}
              className="tw:grid tw:min-h-36 tw:w-full tw:place-content-center tw:justify-items-center tw:gap-1.5 tw:rounded-lg tw:border-2 tw:border-dashed tw:border-border tw:bg-muted/40 tw:p-6 tw:text-center tw:text-foreground tw:outline-none tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:hover:bg-muted tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:disabled:cursor-default tw:disabled:opacity-60 tw:reduce-motion:transition-none">
              <Icon name="import" size={24} className="tw:text-muted-foreground" />
              <span className="tw:text-body tw:font-semibold">{busy === "import" ? "Importing…" : "Choose a file"}</span>
              <span className="tw:text-small tw:text-muted-foreground">.studydeck · Anki .apkg · .ostudy.json</span>
            </ItemTrigger>
            <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
              <span className="tw:text-small tw:text-muted-foreground">Coming from Quizlet? Copy a set&apos;s Export text.</span>
              <Button variant="secondary" disabled={busy !== null} onClick={() => { setPasting(true); setErrorMessage(null); }}>Paste from Quizlet</Button>
            </div>
            {subjects.length > 0 ? (
              <FormField label="Into subject">
                <NativeSelect value={intoSubjectId ?? ""} disabled={busy !== null} onChange={(event) => setIntoSubjectId(event.currentTarget.value || null)}>
                  <option value="">No subject</option>
                  {subjects.map((subject) => <option key={subject.id} value={subject.id}>{subject.name}</option>)}
                </NativeSelect>
              </FormField>
            ) : null}
          </> : <Panel title="Decks to export" aria-label="Decks to export">
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

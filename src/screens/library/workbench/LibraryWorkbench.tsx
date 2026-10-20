import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { ShowToast } from "../../../components/Toast";
import { reconcileCommitted } from "../../../controllers/reconciliation";
import { DecksController } from "../../../controllers/decksController";
import type { AuthoringNote } from "../../../ipc/authoringNotes";
import { isQuestionNote } from "../../../ipc/authoringNotes";
import type { CardBrowserRow, CardSort, SelectionCard } from "../../../ipc/cards";
import { createMutationId, createOperationId, getErrorCode, getErrorMessage } from "../../../ipc/common";
import type { DeckSummary, MergeDecksInput } from "../../../ipc/decks";
import type { DocumentSummary } from "../../../ipc/documents";
import type { SelectedPracticeRequest } from "../../../ipc/practice";
import type { ScheduleAction } from "../../../ipc/scheduling";
import { getSubjectLibrary, moveDeckToSubject } from "../../../ipc/subjects";
import { registerPlatformBackHandler } from "../../../platform/backNavigation";
import type { OverflowMenuItem } from "../../../ui/overlays/OverflowMenu";
import { OverflowMenu } from "../../../ui/overlays/OverflowMenu";
import { useConfirm } from "../../../ui/overlays/overlayContext";
import { useLayoutMode } from "../../../ui/layout/LayoutModeProvider";
import { cn } from "../../../ui/internal/cn";
import { ResizablePanes } from "../../../ui/patterns/ResizablePanes";
import type { ResponsiveAction } from "../../../ui/patterns/ResponsiveActionGroup";
import { IconButton } from "../../../ui/icons/IconButton";
import { Button } from "../../../ui/primitives/Button";
import { InlineNotice, InlineProgress } from "../../../ui/primitives/InlineNotice";
import { CardEditorScreen } from "../../CardEditorScreen";
import { BulkEntryModal } from "../../cards/BulkEntryModal";
import { BulkTagModal } from "../../cards/BulkTagModal";
import { DeleteCardsModal } from "../../cards/DeleteCardsModal";
import { MoveCopyModal, type TransferableCard } from "../../cards/MoveCopyModal";
import { PracticeResultsModal } from "../../cards/PracticeResultsModal";
import { ScheduleActionModal } from "../../cards/ScheduleActionModal";
import { scheduleActionNotice } from "../../cards/scheduleActionResult";
import { DeckMergeSheet } from "../../flashcards/DeckMergeSheet";
import { DeckTransferDialog, type DeckTransferDialogRequest } from "../../flashcards/DeckTransferDialog";
import { PracticeSetupSheet } from "../../flashcards/deck-library/PracticeSetupSheet";
import type { PracticeDeckScope } from "../../flashcards/deck-library/practiceSetupModel";
import { CheckTheseSheet } from "../../home/CheckTheseSheet";
import { useLibraryFullPage, useLibraryTabActions } from "../LibraryFrame";
import { ALL_SUBJECTS, type LibrarySubjectScope } from "../../../navigation/appDestinations";
import { NewSubjectSheet } from "../../home/NewSubjectSheet";
import { CardScheduleSummary } from "./CardScheduleSummary";
import { useWorkbenchCards, PAGE_SIZE } from "./useWorkbenchCards";
import { useWorkbenchLibrary } from "./useWorkbenchLibrary";
import { WorkbenchCardList } from "./WorkbenchCardList";
import { MoveToSubjectSheet, NameSheet } from "./WorkbenchSheets";
import { WorkbenchTree } from "./WorkbenchTree";
import {
  CARD_FILTERS,
  decksInSubject,
  DEFAULT_SCOPE,
  defaultSort,
  sameScope,
  scopeExists,
  scopeTitle,
  searchInputFor,
  subjectOfDeck,
  uniqueNotes,
  type CardFilter,
  type WorkbenchScope,
} from "./workbenchModel";

/** Where Library opens: the shell passes the subject, deck or search it was asked for. */
export interface WorkbenchLaunch {
  /** Changes for every request, so asking for the same place again still applies it. */
  requestId: number;
  scope?: WorkbenchScope;
  focusSearch?: boolean;
}

export interface LibraryWorkbenchProps {
  decks: DeckSummary[];
  refreshDecks: () => Promise<DeckSummary[]>;
  launch?: WorkbenchLaunch;
  onPracticeDeck: (deckId: string) => void | Promise<void>;
  onPracticeCards: (cardIds: string[]) => void | Promise<void>;
  /** Practice with options: scheduled or extra cards, tags and shuffle. */
  onPracticeSelected: (request: SelectedPracticeRequest) => void | Promise<void>;
  practiceShuffle: boolean;
  onOpenDocuments: () => void;
  onCardDirtyChange: (dirty: boolean) => void;
  /** The open deck, or null for any other place, so practice can return to it. */
  onDeckScopeChange?: (deckId: string | null) => void;
  /** Library's chosen subject: the tree shows only it, and a subject chosen here becomes Library's. */
  subject?: LibrarySubjectScope;
  onSubjectChange?: (subject: LibrarySubjectScope) => void;
  showToast: ShowToast;
}

type Sheet =
  | { kind: "new-subject" }
  | { kind: "rename-subject"; subjectId: string; name: string }
  | { kind: "new-deck"; subjectId: string | null }
  | { kind: "rename-deck"; deck: DeckSummary }
  | { kind: "move-deck"; deck: DeckSummary }
  | { kind: "move-document"; document: DocumentSummary }
  | { kind: "merge-deck"; source: DeckSummary; operationId: string }
  | { kind: "transfer"; request: DeckTransferDialogRequest }
  | { kind: "check-these" }
  | { kind: "bulk-add"; deckId: string }
  | { kind: "move-cards"; cards: TransferableCard[] }
  | { kind: "tag-cards"; mode: "add" | "remove" }
  | { kind: "delete-cards" }
  | { kind: "schedule-cards"; action: "reschedule" | "reset"; cards: SelectionCard[] }
  | { kind: "practice-cards" }
  | { kind: "practice-setup"; scope: PracticeDeckScope };

type Editor =
  | { kind: "card"; row: CardBrowserRow; note: AuthoringNote }
  | { kind: "new"; deckId: string };

type CompactPage = "tree" | "cards" | "editor";

const PANE_STORAGE_KEY = "ostudy.workbench.panes";
const MAX_SUBJECT_NAME = 120;
const MAX_DECK_NAME = 200;

/**
 * Library as one workbench: subjects, decks and tags on the left, the chosen
 * cards in the middle and the open card's editor on the right. Wide screens
 * show the three side by side with resizable panes; medium screens show the
 * editor in place of the list; phones show one page at a time.
 */
export function LibraryWorkbench({
  decks,
  refreshDecks,
  launch,
  onPracticeDeck,
  onPracticeCards,
  onPracticeSelected,
  practiceShuffle,
  onOpenDocuments,
  onCardDirtyChange,
  onDeckScopeChange,
  subject,
  onSubjectChange,
  showToast,
}: LibraryWorkbenchProps) {
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const confirm = useConfirm();
  const libraryState = useWorkbenchLibrary(decks, refreshDecks, showToast);
  const { library, tags } = libraryState;
  const [scope, setScope] = useState<WorkbenchScope>(launch?.scope ?? DEFAULT_SCOPE);
  const [filter, setFilter] = useState<CardFilter>("all");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<CardSort>(() => defaultSort(launch?.scope ?? DEFAULT_SCOPE));
  const [editor, setEditor] = useState<Editor | null>(null);
  const [editorDirty, setEditorDirty] = useState(false);
  const [opening, setOpening] = useState(false);
  const [sheet, setSheet] = useState<Sheet | null>(null);
  const [page, setPage] = useState<CompactPage>(launch?.scope ? "cards" : "tree");
  const [scheduleBusy, setScheduleBusy] = useState(false);
  const [paneSizes, setPaneSizes] = useState<Record<string, number>>(readPaneSizes);
  const searchRef = useRef<HTMLInputElement | null>(null);
  const pendingSchedule = useRef<{ key: string; mutationId: string } | null>(null);
  const mergeRequest = useRef<MergeDecksInput | null>(null);
  const decksControllerRef = useRef<DecksController | null>(null);
  if (decksControllerRef.current === null) decksControllerRef.current = new DecksController();
  const decksController = decksControllerRef.current;

  const tagName = useCallback((tagId: string) => tags.find((tag) => tag.id === tagId)?.name, [tags]);
  const input = useMemo(
    () => searchInputFor(scope, filter, query, sort, library, PAGE_SIZE),
    [filter, library, query, scope, sort],
  );
  const countInputs = useMemo(() => {
    const entries = CARD_FILTERS.map((option) => [option.value, searchInputFor(scope, option.value, query, sort, library, 1)] as const)
      .filter((entry): entry is readonly [CardFilter, NonNullable<(typeof entry)[1]>] => entry[1] !== null);
    return entries.length > 0 ? Object.fromEntries(entries) : null;
  }, [library, query, scope, sort]);
  const cards = useWorkbenchCards(input, countInputs);
  const selected = useMemo(() => [...cards.selected.values()], [cards.selected]);
  const activeDecks = useMemo(() => decks.filter((deck) => deck.archivedAt === null), [decks]);
  const scopeDeck = scope.kind === "deck" ? decks.find((deck) => deck.id === scope.deckId) ?? null : null;

  const scopeDeckId = scope.kind === "deck" ? scope.deckId : null;
  useEffect(() => {
    onDeckScopeChange?.(scopeDeckId);
  }, [onDeckScopeChange, scopeDeckId]);

  // Choosing All cards or a subject here chooses it for the rest of Library too.
  const reportedSubject: LibrarySubjectScope | null =
    scope.kind === "view" && scope.view === "all"
      ? ALL_SUBJECTS
      : scope.kind === "subject"
        ? scope.subjectId === null ? { kind: "none" } : { kind: "subject", subjectId: scope.subjectId }
        : null;
  const reportedSubjectKey = reportedSubject === null ? null : JSON.stringify(reportedSubject);
  useEffect(() => {
    if (reportedSubjectKey !== null) onSubjectChange?.(JSON.parse(reportedSubjectKey) as LibrarySubjectScope);
  }, [onSubjectChange, reportedSubjectKey]);

  // A launch from Home, a subject's Manage button or the header's Search.
  const appliedLaunch = useRef<number | null>(launch?.requestId ?? null);
  useEffect(() => {
    if (!launch || appliedLaunch.current === launch.requestId) return;
    appliedLaunch.current = launch.requestId;
    if (launch.scope) chooseScope(launch.scope, { force: true });
    if (launch.focusSearch) {
      setPage("cards");
      window.setTimeout(() => searchRef.current?.focus(), 0);
    }
    // chooseScope is stable in effect: it reads only setters and current state.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [launch]);
  useEffect(() => {
    if (launch?.focusSearch) searchRef.current?.focus();
    // Only on the first mount; later launches focus above.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Android Back steps out of the Workbench before leaving Library: on phones
  // from the card list to the tree, elsewhere from a deck, subject or tag to All
  // cards. The open card's editor handles Back itself.
  const backAction = useRef<() => void>(() => undefined);
  backAction.current = compact ? () => setPage("tree") : () => void chooseScope(DEFAULT_SCOPE);
  const backActive = compact ? page === "cards" : !sameScope(scope, DEFAULT_SCOPE);
  useEffect(() => {
    if (!backActive) return;
    return registerPlatformBackHandler(() => {
      backAction.current();
      return true;
    });
  }, [backActive]);

  // A deck or subject that disappears (deleted here or elsewhere) returns the list to All cards.
  useEffect(() => {
    if (libraryState.loading || !library.organization) return;
    if (!scopeExists(scope, library)) {
      setScope(DEFAULT_SCOPE);
      setSort(defaultSort(DEFAULT_SCOPE));
    }
  }, [library, libraryState.loading, scope]);

  useEffect(() => { onCardDirtyChange(editorDirty); }, [editorDirty, onCardDirtyChange]);
  useEffect(() => () => onCardDirtyChange(false), [onCardDirtyChange]);
  useEffect(() => () => decksController.invalidate(), [decksController]);

  async function canLeaveEditor(): Promise<boolean> {
    if (!editorDirty) return true;
    return confirm({
      title: "Discard changes to this card?",
      description: "The card has edits that are not saved.",
      confirmLabel: "Discard changes",
      destructive: true,
    });
  }

  function closeEditor(): void {
    setEditorDirty(false);
    setEditor(null);
    if (compact || layoutMode === "medium") setPage("cards");
  }

  async function chooseScope(next: WorkbenchScope, options: { force?: boolean } = {}): Promise<void> {
    if (!options.force && sameScope(next, scope)) {
      setPage("cards");
      return;
    }
    if (!(await canLeaveEditor())) return;
    setScope(next);
    setFilter("all");
    setQuery("");
    setSort(defaultSort(next));
    setEditorDirty(false);
    setEditor(null);
    setPage("cards");
  }

  async function openCard(row: CardBrowserRow): Promise<void> {
    if (editor?.kind === "card" && editor.row.cardId === row.cardId) {
      setPage("editor");
      return;
    }
    if (!(await canLeaveEditor())) return;
    setOpening(true);
    try {
      const note = await cards.controller.note(row.noteId, row.cardType);
      if (!note) return;
      setEditorDirty(false);
      setEditor({ kind: "card", row, note });
      setPage("editor");
    } catch (caught: unknown) {
      showToast("error", getErrorMessage(caught));
    } finally {
      setOpening(false);
    }
  }

  async function beginNewCard(deckId: string): Promise<void> {
    if (!(await canLeaveEditor())) return;
    setEditorDirty(false);
    setEditor({ kind: "new", deckId });
    setPage("editor");
  }

  const refreshAfterMutation = useCallback(async () => {
    await Promise.all([cards.reload(), libraryState.reload()]);
  }, [cards, libraryState]);

  async function reconcileAfterCommit(subject: string): Promise<void> {
    const outcome = await reconcileCommitted(undefined, refreshAfterMutation);
    if (outcome.reconciliationError) {
      showToast("info", `${subject} was saved, but the card list could not refresh: ${outcome.reconciliationError}`);
    }
  }

  async function completeCardMutation(): Promise<void> {
    if (editor) closeEditor();
    await refreshAfterMutation();
  }

  async function applySchedule(targets: SelectionCard[], action: Exclude<ScheduleAction, "reset" | "reschedule">): Promise<void> {
    if (targets.length === 0 || scheduleBusy) return;
    const cardIds = [...new Set(targets.map((card) => card.cardId))].sort();
    const key = JSON.stringify({ action, cardIds });
    // Retrying the same action on the same cards reuses its identity, so a lost answer cannot apply it twice.
    if (pendingSchedule.current?.key !== key) pendingSchedule.current = { key, mutationId: createMutationId() };
    const mutationId = pendingSchedule.current.mutationId;
    setScheduleBusy(true);
    try {
      const result = await cards.controller.schedule({ mutationId, cardIds, action });
      pendingSchedule.current = null;
      const notice = scheduleActionNotice(result, cardIds.length);
      showToast(notice.kind, notice.text);
    } catch (caught: unknown) {
      if (getErrorCode(caught) !== null) {
        pendingSchedule.current = null;
        showToast("error", getErrorMessage(caught));
      } else {
        showToast("error", `${getErrorMessage(caught)} Choose the same action again to safely retry it.`);
      }
      setScheduleBusy(false);
      return;
    }
    await reconcileAfterCommit("The schedule change");
    // Keep the open card's summary in step with its new schedule.
    if (editor?.kind === "card" && cardIds.includes(editor.row.cardId)) {
      setEditor((current) => {
        if (current?.kind !== "card") return current;
        const fresh = cards.rows.find((candidate) => candidate.cardId === current.row.cardId);
        return fresh ? { ...current, row: fresh } : current;
      });
    }
    setScheduleBusy(false);
  }

  // Keep the open card's row (its schedule summary) current after a reload.
  useEffect(() => {
    if (editor?.kind !== "card") return;
    const fresh = cards.rows.find((candidate) => candidate.cardId === editor.row.cardId);
    if (fresh && fresh !== editor.row) setEditor({ ...editor, row: fresh });
  }, [cards.rows, editor]);

  async function createDeckIn(subjectId: string | null, name: string): Promise<boolean> {
    try {
      const deck = await decksController.create({ name });
      if (subjectId !== null) await libraryState.moveDecks([deck.id], subjectId);
      await libraryState.reload();
      showToast("success", `Created “${deck.name}”.`);
      await chooseScope({ kind: "deck", deckId: deck.id }, { force: true });
      return true;
    } catch (caught: unknown) {
      showToast("error", getErrorMessage(caught));
      return false;
    }
  }

  async function renameDeck(deck: DeckSummary, name: string): Promise<boolean> {
    try {
      await decksController.update({ deckId: deck.id, name, description: deck.description, revision: deck.revision });
      await refreshDecks();
      return true;
    } catch (caught: unknown) {
      showToast("error", getErrorMessage(caught));
      return false;
    }
  }

  async function setArchived(deck: DeckSummary, archived: boolean): Promise<void> {
    try {
      await decksController.setArchived({ deckId: deck.id, revision: deck.revision, archived });
      showToast("success", archived ? `Archived “${deck.name}”.` : `Restored “${deck.name}”.`);
      await refreshAfterMutation();
    } catch (caught: unknown) {
      showToast("error", getErrorMessage(caught));
    }
  }

  async function deleteDeck(deck: DeckSummary): Promise<void> {
    const accepted = await confirm({
      title: `Delete “${deck.name}”?`,
      description: `This will delete ${deck.generatedCardCount.toLocaleString()} ${deck.generatedCardCount === 1 ? "card" : "cards"} and their review history after the Undo period.`,
      confirmLabel: "Delete deck",
      destructive: true,
    });
    if (!accepted) return;
    try {
      const deletion = await decksController.deleteDeck(deck.id);
      showToast("success", `Deleted “${deck.name}”.`, {
        label: "Undo",
        expiresAt: deletion.undoExpiresAt,
        run: async () => {
          try {
            await decksController.undo(deletion.undoToken);
            await refreshAfterMutation();
          } catch (caught: unknown) {
            showToast("error", getErrorMessage(caught));
            throw caught;
          }
        },
      });
      if (scope.kind === "deck" && scope.deckId === deck.id) {
        setScope(DEFAULT_SCOPE);
        setSort(defaultSort(DEFAULT_SCOPE));
      }
      await refreshAfterMutation();
    } catch (caught: unknown) {
      showToast("error", getErrorMessage(caught));
    }
  }

  function beginMerge(deck: DeckSummary): void {
    if (!activeDecks.some((candidate) => candidate.id !== deck.id)) {
      showToast("info", "Create or restore another active deck before merging this deck.");
      return;
    }
    mergeRequest.current = null;
    setSheet({ kind: "merge-deck", source: deck, operationId: createOperationId() });
  }

  async function mergeDeck(source: DeckSummary, operationId: string, destinationDeckId: string): Promise<void> {
    const destination = activeDecks.find((deck) => deck.id === destinationDeckId);
    if (!destination) return;
    let request = mergeRequest.current;
    if (!request) {
      request = {
        operationId,
        sourceDeckId: source.id,
        destinationDeckId: destination.id,
        sourceRevision: source.revision,
        destinationRevision: destination.revision,
        expectedSourceNoteCount: source.sourceNoteCount,
        expectedSourceCardCount: source.generatedCardCount,
      };
      mergeRequest.current = request;
    } else if (request.destinationDeckId !== destination.id) {
      showToast("info", "The first merge attempt already captured its destination. Close and reopen Merge to choose another deck.");
      return;
    }
    try {
      const result = await decksController.merge(request);
      mergeRequest.current = null;
      setSheet(null);
      showToast("success", `Merged ${result.movedCardCount.toLocaleString()} ${result.movedCardCount === 1 ? "card" : "cards"} from “${source.name}” into “${destination.name}”. “${source.name}” is now archived.`);
      await chooseScope({ kind: "deck", deckId: destination.id }, { force: true });
      await reconcileAfterCommit("The merge");
    } catch (caught: unknown) {
      showToast("error", getErrorMessage(caught));
    }
  }

  function deckMenu(deck: DeckSummary): OverflowMenuItem[] {
    if (deck.archivedAt !== null) {
      return [
        { id: "restore", label: "Restore", onSelect: () => void setArchived(deck, false) },
        { id: "export", label: "Export…", onSelect: () => setSheet({ kind: "transfer", request: { kind: "export", decks: [deck] } }) },
        { id: "delete", label: "Delete deck", destructive: true, onSelect: () => void deleteDeck(deck) },
      ];
    }
    return [
      { id: "practice", label: "Practice", disabled: deck.generatedCardCount === 0, onSelect: () => void onPracticeDeck(deck.id) },
      { id: "practice-options", label: "Practice with options…", disabled: deck.generatedCardCount === 0, onSelect: () => beginPracticeSetup([deck]) },
      { id: "rename", label: "Rename…", onSelect: () => setSheet({ kind: "rename-deck", deck }) },
      { id: "move", label: "Move to subject…", onSelect: () => setSheet({ kind: "move-deck", deck }) },
      { id: "up", label: "Move up", onSelect: () => void moveDeckBy(deck, -1) },
      { id: "down", label: "Move down", onSelect: () => void moveDeckBy(deck, 1) },
      { id: "merge", label: "Merge into…", disabled: deck.sourceNoteCount === 0, onSelect: () => beginMerge(deck) },
      { id: "export", label: "Export…", onSelect: () => setSheet({ kind: "transfer", request: { kind: "export", decks: [deck] } }) },
      { id: "archive", label: "Archive", onSelect: () => void setArchived(deck, true) },
      { id: "delete", label: "Delete deck", destructive: true, onSelect: () => void deleteDeck(deck) },
    ];
  }

  function beginPracticeSetup(scopeDecks: readonly DeckSummary[]): void {
    const studyable = scopeDecks.filter((deck) => deck.archivedAt === null && deck.generatedCardCount > 0);
    if (studyable.length === 0) {
      showToast("info", "These decks have no cards to practice yet.");
      return;
    }
    const snapshots = studyable
      .map((deck) => ({ id: deck.id, name: deck.name, revision: deck.revision, generatedCardCount: deck.generatedCardCount }))
      .sort((left, right) => left.id.localeCompare(right.id));
    setSheet({ kind: "practice-setup", scope: { deckIds: snapshots.map((deck) => deck.id), decks: snapshots } });
  }

  async function moveDeckBy(deck: DeckSummary, offset: -1 | 1): Promise<void> {
    const subjectId = subjectOfDeck(library, deck.id);
    const siblings = decksInSubject(library, subjectId);
    const index = siblings.findIndex((candidate) => candidate.id === deck.id);
    const target = index + offset;
    if (index < 0 || target < 0 || target >= siblings.length) return;
    try {
      const current = await getSubjectLibrary();
      libraryState.acceptOrganization(await moveDeckToSubject({
        deckId: deck.id,
        subjectId,
        targetIndex: target,
        expectedLibraryRevision: current.libraryRevision,
      }));
    } catch (caught: unknown) {
      showToast("error", getErrorMessage(caught));
    }
  }

  function documentMenu(document: DocumentSummary): OverflowMenuItem[] {
    return [
      { id: "open", label: "Open in Documents", onSelect: onOpenDocuments },
      { id: "move", label: "Move to subject…", onSelect: () => setSheet({ kind: "move-document", document }) },
    ];
  }

  async function deleteSubject(subjectId: string): Promise<void> {
    const subject = library.subjects.find((candidate) => candidate.id === subjectId);
    if (!subject) return;
    const accepted = await confirm({
      title: `Delete “${subject.name}”?`,
      description: "Its decks and documents move to No subject, with their cards and schedules. The subject's overview guide is deleted.",
      confirmLabel: "Delete subject",
      destructive: true,
    });
    if (!accepted) return;
    if (await libraryState.deleteSubject(subjectId)) showToast("success", `Deleted “${subject.name}”.`);
  }

  async function moveSubjectBy(subjectId: string, offset: -1 | 1): Promise<void> {
    const ordered = [...library.subjects].sort((left, right) => left.sortOrder - right.sortOrder);
    const index = ordered.findIndex((subject) => subject.id === subjectId);
    const target = index + offset;
    if (index < 0 || target < 0 || target >= ordered.length) return;
    await libraryState.moveSubject(subjectId, target);
  }

  const selectionActions = useMemo((): { primary: Omit<ResponsiveAction, "placement" | "presentation">; secondary: ResponsiveAction[] } => {
    const notes = uniqueNotes(selected);
    const run = (action: () => void) => () => action();
    return {
      primary: { id: "move", label: "Move…", icon: "move", onSelect: run(() => setSheet({ kind: "move-cards", cards: notes })) },
      secondary: [
        { id: "tag", label: "Tag…", icon: "plus", placement: "wide-and-medium", presentation: "text", onSelect: run(() => setSheet({ kind: "tag-cards", mode: "add" })) },
        { id: "suspend", label: "Suspend", placement: "wide-and-medium", presentation: "text", disabled: scheduleBusy, onSelect: run(() => void applySchedule(selected, "suspend")) },
        { id: "practice", label: "Practice…", placement: "overflow-only", onSelect: run(() => setSheet({ kind: "practice-cards" })) },
        { id: "untag", label: "Remove tags…", placement: "overflow-only", onSelect: run(() => setSheet({ kind: "tag-cards", mode: "remove" })) },
        { id: "resume", label: "Resume", placement: "overflow-only", disabled: scheduleBusy, onSelect: run(() => void applySchedule(selected, "resume")) },
        { id: "bury", label: "Bury until tomorrow", placement: "overflow-only", disabled: scheduleBusy, onSelect: run(() => void applySchedule(selected, "bury")) },
        { id: "unbury", label: "Unbury", placement: "overflow-only", disabled: scheduleBusy, onSelect: run(() => void applySchedule(selected, "unbury")) },
        { id: "reschedule", label: "Reschedule…", placement: "overflow-only", onSelect: run(() => setSheet({ kind: "schedule-cards", action: "reschedule", cards: selected })) },
        { id: "reset", label: "Reset schedules…", placement: "overflow-only", destructive: true, onSelect: run(() => setSheet({ kind: "schedule-cards", action: "reset", cards: selected })) },
        { id: "delete", label: "Delete cards…", placement: "overflow-only", destructive: true, onSelect: run(() => setSheet({ kind: "delete-cards" })) },
      ],
    };
    // applySchedule reads current state each render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scheduleBusy, selected]);

  const title = scopeTitle(scope, library, tagName);
  const eyebrow = scope.kind === "deck"
    ? (() => {
        const subjectId = subjectOfDeck(library, scope.deckId);
        const subjectName = subjectId === null ? "No subject" : library.subjects.find((subject) => subject.id === subjectId)?.name;
        return scopeDeck?.archivedAt ? `${subjectName ?? "Library"} · Archived deck` : (subjectName ?? undefined);
      })()
    : scope.kind === "subject" ? "Subject" : scope.kind === "tag" ? "Tag" : "Library";

  const headerActions = scope.kind === "deck" && scopeDeck ? (
    <>
      {scopeDeck.archivedAt === null ? (
        <>
          <Button variant="secondary" size="small" disabled={scopeDeck.generatedCardCount === 0} onClick={() => void onPracticeDeck(scopeDeck.id)}>Practice</Button>
          <Button variant="primary" size="small" onClick={() => void beginNewCard(scopeDeck.id)}>New card</Button>
        </>
      ) : null}
      <OverflowMenu
        label={`${scopeDeck.name} actions`}
        items={[
          ...(scopeDeck.archivedAt === null
            ? [{ id: "bulk", label: "Bulk add…", onSelect: () => setSheet({ kind: "bulk-add", deckId: scopeDeck.id }) }]
            : []),
          ...deckMenu(scopeDeck).filter((item) => item.id !== "practice" && item.id !== "up" && item.id !== "down"),
        ]}
      />
    </>
  ) : scope.kind === "subject" ? (
    <>
      <Button
        variant="secondary"
        size="small"
        disabled={decksInSubject(library, scope.subjectId).every((deck) => deck.generatedCardCount === 0)}
        onClick={() => beginPracticeSetup(decksInSubject(library, scope.subjectId))}
      >
        Practice…
      </Button>
      <Button variant="secondary" size="small" onClick={() => setSheet({ kind: "new-deck", subjectId: scope.subjectId })}>New deck</Button>
    </>
  ) : null;

  const empty = emptyCopy(scope, filter, query, scopeDeck);

  const tree = (
    <div className={compact ? "tw:min-w-0" : "tw:h-full tw:min-h-0 tw:overflow-y-auto tw:pr-2"}>
      {libraryState.error ? <InlineNotice kind="error">{libraryState.error}</InlineNotice> : null}
      <WorkbenchTree
        library={library}
        tags={tags}
        scope={scope}
        checkTheseCount={library.organization?.checkTheseCount ?? 0}
        compact={compact}
        onlySubjectId={subject === undefined || subject.kind === "all" ? undefined : subject.kind === "none" ? null : subject.subjectId}
        onSelectScope={(next) => void chooseScope(next)}
        onNewSubject={() => setSheet({ kind: "new-subject" })}
        onRenameSubject={(subjectId) => setSheet({ kind: "rename-subject", subjectId, name: library.subjects.find((subject) => subject.id === subjectId)?.name ?? "" })}
        onMoveSubject={(subjectId, offset) => void moveSubjectBy(subjectId, offset)}
        onDeleteSubject={(subjectId) => void deleteSubject(subjectId)}
        onNewDeck={(subjectId) => setSheet({ kind: "new-deck", subjectId })}
        deckMenu={deckMenu}
        documentMenu={documentMenu}
        onOpenDocument={() => onOpenDocuments()}
        onCheckThese={() => setSheet({ kind: "check-these" })}
      />
    </div>
  );

  const list = (
    <WorkbenchCardList
      cards={cards}
      eyebrow={eyebrow}
      title={title}
      actions={headerActions}
      filter={filter}
      onFilterChange={setFilter}
      query={query}
      onQueryChange={(value) => {
        setQuery(value);
        if (value.trim() !== "" && query.trim() === "") setSort("relevance");
        if (value.trim() === "" && sort === "relevance") setSort(defaultSort(scope));
      }}
      sort={sort}
      onSortChange={setSort}
      searchRef={searchRef}
      openCardId={editor?.kind === "card" ? editor.row.cardId : null}
      onOpenCard={(row) => void openCard(row)}
      selectionActions={selectionActions}
      emptyTitle={empty.title}
      emptyDescription={empty.description}
      emptyAction={query.trim() !== "" || filter !== "all" ? undefined
        : scope.kind === "deck" && scopeDeck && scopeDeck.archivedAt === null ? (
          <Button variant="primary" onClick={() => void beginNewCard(scopeDeck.id)}>New card</Button>
        ) : scope.kind === "view" && scope.view === "all" && activeDecks.length === 0 && !compact ? (
          <>
            <Button variant="primary" onClick={() => setSheet({ kind: "new-deck", subjectId: null })}>New deck</Button>
            <Button variant="secondary" onClick={() => setSheet({ kind: "transfer", request: { kind: "import" } })}>Import deck</Button>
          </>
        ) : undefined}
      compact={compact}
      onBack={compact ? () => setPage("tree") : undefined}
    />
  );

  const editorPane = editor ? (
    <WorkbenchEditor
      editor={editor}
      decks={activeDecks}
      todayStartAt={cards.todayStartAt}
      scheduleBusy={scheduleBusy}
      onDirtyChange={setEditorDirty}
      onClose={closeEditor}
      onSchedule={(action) => {
        if (editor.kind !== "card") return;
        const card = { cardId: editor.row.cardId, noteId: editor.row.noteId, deckId: editor.row.deckId };
        if (action === "reset") setSheet({ kind: "schedule-cards", action: "reset", cards: [card] });
        else void applySchedule([card], action);
      }}
      onMoveCopy={() => {
        if (editor.kind === "card") setSheet({ kind: "move-cards", cards: [{ noteId: editor.row.noteId, deckId: editor.row.deckId }] });
      }}
      onSaved={async (message, closed) => {
        setEditorDirty(false);
        if (closed) closeEditor();
        showToast("success", message);
        await reconcileAfterCommit("The card");
      }}
    />
  ) : null;

  // Import deck sits at the end of Library's tabs while the Workbench is open.
  const importAction = useMemo(
    () => compact ? (
      <IconButton icon="import" label="Import deck" onClick={() => setSheet({ kind: "transfer", request: { kind: "import" } })} />
    ) : (
      <Button variant="secondary" size="small" onClick={() => setSheet({ kind: "transfer", request: { kind: "import" } })}>
        Import deck
      </Button>
    ),
    [compact],
  );
  useLibraryTabActions(importAction);
  // A phone's card editor is a page of its own, without Library's header and tabs.
  useLibraryFullPage(compact && page === "editor");

  let body;
  if (compact) {
    body = page === "editor" && editorPane ? editorPane : page === "cards" ? list : tree;
  } else {
    // The editor opens as a panel over the cards. The list stays mounted
    // underneath (hidden, which keeps its layout), so closing the editor
    // returns to the same scroll position, search, filter and selection.
    const editing = editorPane !== null && (layoutMode !== "medium" || page === "editor");
    const main = (
      <div className="tw:relative tw:flex tw:h-full tw:min-h-0 tw:min-w-0 tw:flex-col">
        <div className={cn("tw:isolate tw:flex tw:h-full tw:min-h-0 tw:min-w-0 tw:flex-col", editing && "tw:invisible")} inert={editing} aria-hidden={editing || undefined}>
          {list}
        </div>
        {editing ? (
          <div
            data-slot="workbench-editor-panel"
            className="tw:absolute tw:inset-0 tw:z-(--ostudy-layer-raised) tw:overflow-y-auto tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-background tw:shadow-lg"
          >
            {editorPane}
          </div>
        ) : null}
      </div>
    );
    const wide = layoutMode !== "medium";
    body = (
      <ResizablePanes
        sizes={paneSizes}
        onSizesChange={(next) => setPaneSizes(savePaneSizes({ ...paneSizes, ...next }))}
        panes={[
          { id: "tree", label: "library list", defaultSize: wide ? 260 : 240, minSize: 200, maxSize: wide ? 400 : 360, content: tree, className: "tw:pr-1" },
          { id: "list", label: "cards", minSize: 400, content: main, className: "tw:pl-3" },
        ]}
      />
    );
  }

  return (
    <div
      data-testid="library-workbench"
      className={compact ? "tw:min-w-0" : "tw:flex tw:h-full tw:min-h-0 tw:flex-1 tw:flex-col"}
    >
      {opening ? <span className="sr-only" role="status">Opening card…</span> : null}
      {libraryState.loading && !library.organization ? <InlineProgress>Loading your library…</InlineProgress> : null}
      <div className={compact ? "tw:min-w-0" : "tw:flex tw:min-h-0 tw:flex-1"}>{body}</div>
      {renderSheet()}
    </div>
  );

  function renderSheet() {
    if (!sheet) return null;
    const close = () => setSheet(null);
    switch (sheet.kind) {
      case "new-subject":
        return (
          <NewSubjectSheet
            onClose={close}
            onCreated={(organization, name) => {
              libraryState.acceptOrganization(organization);
              close();
              showToast("success", `Created “${name}”.`);
              const created = organization.subjects.find((subject) => subject.name === name);
              if (created) void chooseScope({ kind: "subject", subjectId: created.id }, { force: true });
            }}
          />
        );
      case "rename-subject":
        return (
          <NameSheet
            title="Rename subject"
            label="Name"
            initialName={sheet.name}
            submitLabel="Rename"
            maxLength={MAX_SUBJECT_NAME}
            onClose={close}
            onSubmit={(name) => libraryState.renameSubject(sheet.subjectId, name)}
          />
        );
      case "new-deck":
        return (
          <NameSheet
            title="New deck"
            description={sheet.subjectId === null
              ? "The deck starts under No subject. You can move it later."
              : `The deck goes in ${library.subjects.find((subject) => subject.id === sheet.subjectId)?.name ?? "this subject"}.`}
            label="Name"
            submitLabel="Create deck"
            maxLength={MAX_DECK_NAME}
            onClose={close}
            onSubmit={(name) => createDeckIn(sheet.subjectId, name)}
          />
        );
      case "rename-deck":
        return (
          <NameSheet
            title="Rename deck"
            label="Name"
            initialName={sheet.deck.name}
            submitLabel="Rename"
            maxLength={MAX_DECK_NAME}
            onClose={close}
            onSubmit={(name) => renameDeck(sheet.deck, name)}
          />
        );
      case "move-deck":
        return (
          <MoveToSubjectSheet
            itemLabel={`“${sheet.deck.name}”`}
            subjects={library.subjects}
            currentSubjectId={subjectOfDeck(library, sheet.deck.id)}
            onClose={close}
            onMove={(subjectId) => libraryState.moveDecks([sheet.deck.id], subjectId)}
          />
        );
      case "move-document":
        return (
          <MoveToSubjectSheet
            itemLabel={`“${sheet.document.title}”`}
            subjects={library.subjects}
            currentSubjectId={library.organization?.documents.find((placement) => placement.documentId === sheet.document.id)?.subjectId ?? null}
            onClose={close}
            onMove={(subjectId) => libraryState.moveDocument(sheet.document.id, subjectId)}
          />
        );
      case "merge-deck":
        return (
          <DeckMergeSheet
            source={sheet.source}
            decks={decks}
            busy={false}
            onClose={() => { mergeRequest.current = null; close(); }}
            onMerge={(destinationDeckId) => mergeDeck(sheet.source, sheet.operationId, destinationDeckId)}
          />
        );
      case "transfer":
        return (
          <DeckTransferDialog
            request={sheet.request}
            refreshDecks={refreshDecks}
            onImportedDeck={async (outcome) => {
              await libraryState.reload();
              await chooseScope({ kind: "deck", deckId: outcome.deckId }, { force: true });
            }}
            onClose={close}
            showToast={showToast}
          />
        );
      case "check-these":
        return <CheckTheseSheet onClose={close} onChanged={() => void libraryState.reload()} />;
      case "bulk-add":
        return (
          <BulkEntryModal decks={activeDecks} fixedDeckId={sheet.deckId} onClose={close} onSaved={refreshAfterMutation} showToast={showToast} />
        );
      case "move-cards":
        return (
          <MoveCopyModal cards={sheet.cards} decks={activeDecks} onClose={close} onComplete={completeCardMutation} showToast={showToast} />
        );
      case "tag-cards":
        return (
          <BulkTagModal
            cards={uniqueNotes(selected)}
            mode={sheet.mode}
            suggestions={tags}
            onClose={close}
            onComplete={completeCardMutation}
            showToast={showToast}
          />
        );
      case "delete-cards":
        return <DeleteCardsModal cards={uniqueNotes(selected)} onClose={close} onComplete={completeCardMutation} showToast={showToast} />;
      case "schedule-cards":
        return (
          <ScheduleActionModal cards={sheet.cards} action={sheet.action} onClose={close} onComplete={refreshAfterMutation} showToast={showToast} />
        );
      case "practice-setup":
        return (
          <PracticeSetupSheet
            scope={sheet.scope}
            currentDecks={decks}
            initialShuffle={practiceShuffle}
            onClose={close}
            onStart={async (request) => {
              close();
              await onPracticeSelected(request);
            }}
          />
        );
      case "practice-cards":
        return (
          <PracticeResultsModal
            cards={selected}
            sourceLabel={`“${title}”`}
            refreshDecks={refreshDecks}
            onPracticeCards={(cardIds) => void onPracticeCards(cardIds)}
            onClose={close}
            showToast={showToast}
          />
        );
    }
  }
}

interface WorkbenchEditorProps {
  editor: Editor;
  decks: DeckSummary[];
  todayStartAt: number;
  scheduleBusy: boolean;
  onDirtyChange: (dirty: boolean) => void;
  onClose: () => void;
  onSchedule: (action: "suspend" | "resume" | "reset") => void;
  onMoveCopy: () => void;
  onSaved: (message: string, closed: boolean) => Promise<void>;
}

/** The open card in today's card editor, with its schedule below the fields and Suspend and Reset in its menu. */
function WorkbenchEditor({ editor, decks, todayStartAt, scheduleBusy, onDirtyChange, onClose, onSchedule, onMoveCopy, onSaved }: WorkbenchEditorProps) {
  const deckId = editor.kind === "card" ? editor.note.deckId : editor.deckId;
  const deck = decks.find((candidate) => candidate.id === deckId);
  if (!deck) {
    return (
      <div className="tw:grid tw:gap-3 tw:p-2">
        <InlineNotice kind="error">This card's deck is archived or gone. Restore the deck to edit the card.</InlineNotice>
        <Button variant="secondary" onClick={onClose}>Close</Button>
      </div>
    );
  }
  const card = editor.kind === "card" ? editor.note : null;
  return (
    <section aria-label="Card editor" className="tw:grid tw:min-w-0 tw:content-start tw:gap-3 tw:pb-6" data-testid="workbench-editor">
      <CardEditorScreen
        details={editor.kind === "card" ? <CardScheduleSummary row={editor.row} todayStartAt={todayStartAt} /> : undefined}
        moreActions={editor.kind === "card" ? [
          {
            id: "suspend",
            label: editor.row.suspendedAt !== null ? "Resume" : "Suspend",
            disabled: scheduleBusy,
            onSelect: () => onSchedule(editor.row.suspendedAt !== null ? "resume" : "suspend"),
          },
          ...(editor.row.reviewCount > 0
            ? [{ id: "reset", label: "Reset schedule…", destructive: true, disabled: scheduleBusy, onSelect: () => onSchedule("reset") }]
            : []),
        ] : undefined}
        key={editor.kind === "card" ? editor.row.noteId : `new-${editor.deckId}`}
        deck={deck}
        decks={decks}
        card={card}
        onDirtyChange={onDirtyChange}
        closeButton
        onCancel={() => { onDirtyChange(false); onClose(); }}
        onMoveCopy={card && !isQuestionNote(card) ? onMoveCopy : undefined}
        onSaved={(saved, continueAdding) => onSaved(
          saved.deckId !== deck.id ? "Card saved. It is now in a different deck." : card ? "Card updated." : "Card saved.",
          !continueAdding && !card,
        )}
        onQuestionSaved={(_noteId, continueAdding) => onSaved(card ? "Question updated." : "Question saved.", !continueAdding && !card)}
        onTemplateGenerated={(result, continueAdding, template) => onSaved(
          `Generated ${result.createdCount} ${result.createdCount === 1 ? "card" : "cards"} from “${template.name}”.`,
          !continueAdding,
        )}
      />
    </section>
  );
}

function emptyCopy(scope: WorkbenchScope, filter: CardFilter, query: string, deck: DeckSummary | null): { title: string; description: string } {
  if (query.trim() !== "") return { title: "No matching cards", description: "Try other words or clear the search." };
  if (filter !== "all") return { title: "No cards here", description: "No cards match this filter." };
  if (scope.kind === "deck") {
    return deck?.archivedAt
      ? { title: "This deck is archived", description: "Restore it from its menu to study or edit its cards." }
      : { title: "This deck is empty", description: "Add a card, or use Bulk add from the deck's menu." };
  }
  if (scope.kind === "subject") return { title: "No cards in this subject yet", description: "Create a deck here, or move a deck into this subject." };
  if (scope.kind === "view" && scope.view === "due") return { title: "Nothing due today", description: "Every card is scheduled for later." };
  if (scope.kind === "view" && scope.view === "all") return { title: "No cards yet", description: "Create a deck, import one, or add a document to make cards from it." };
  return { title: "No cards here", description: "No cards match this view." };
}

function readPaneSizes(): Record<string, number> {
  try {
    const parsed: unknown = JSON.parse(window.localStorage.getItem(PANE_STORAGE_KEY) ?? "{}");
    if (!parsed || typeof parsed !== "object") return {};
    return Object.fromEntries(Object.entries(parsed).filter((entry): entry is [string, number] => typeof entry[1] === "number"));
  } catch {
    return {};
  }
}

function savePaneSizes(sizes: Record<string, number>): Record<string, number> {
  try {
    window.localStorage.setItem(PANE_STORAGE_KEY, JSON.stringify(sizes));
  } catch {
    // Pane widths are a convenience; the defaults work without storage.
  }
  return sizes;
}

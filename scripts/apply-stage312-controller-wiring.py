#!/usr/bin/env python3
from __future__ import annotations

import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"

DOMAIN_SYMBOLS: dict[str, str] = {}

def domain(name: str, symbols: list[str]) -> None:
    for symbol in symbols:
        DOMAIN_SYMBOLS[symbol] = name

domain("common", ["RichDocument", "DeletionResult", "UndoDeletionResult", "CommandErrorPayload", "createOperationId", "createSubmissionId", "createMutationId", "getErrorCode", "getErrorMessage"])
domain("assets", ["Asset", "AssetDraft", "beginAssetDraft", "cancelAssetDraft", "syncAssetDraft", "importImage"])
domain("decks", ["DeckSummary", "CreateDeckInput", "UpdateDeckInput", "SetDeckArchivedInput", "DeckCardSummaryQuery", "DeckCardSummary", "DeckCardSummaryPage", "ExportResult", "ImportResult", "listDecks", "getDeck", "createDeck", "updateDeck", "setDeckArchived", "deleteDeck", "listDeckCardSummaries", "exportDeck", "importDeck"])
domain("cards", ["BuiltInCardType", "ClozeMode", "CardNote", "CardPresentation", "CreateCardNoteInput", "UpdateCardNoteInput", "TagSummary", "AvailabilityFilter", "CardStatusFilter", "ReviewedFilter", "CardSort", "CardSearchInput", "CardBrowserRow", "CardSearchPage", "SelectionCard", "DuplicateCheckInput", "DuplicateCard", "BulkCardRowInput", "AnalyzeBulkCardsInput", "BulkAnalyzeResult", "BulkCreateCardsInput", "BulkCreateResult", "TransferCardsMode", "TransferCardsInput", "TransferCardsResult", "TagChangeMode", "BulkTagCardsInput", "BulkActionResult", "listTags", "searchCards", "listMatchingCards", "getCardNote", "createCardNote", "updateCardNote", "findDuplicateCards", "analyzeBulkCards", "bulkCreateCards", "transferCards", "bulkTagCards", "deleteCard", "deleteCards", "undoDeletion"])
domain("templates", ["CardTemplateField", "CardTemplateRule", "CardTemplate", "CardTemplateSummary", "CardTemplateFieldInput", "CardTemplateRuleInput", "CreateCardTemplateInput", "UpdateCardTemplateInput", "TemplateValueInput", "TemplateCardPreview", "PreviewTemplateCardsInput", "GenerateTemplateCardsInput", "GenerateTemplateCardsResult", "listCardTemplateSummaries", "getCardTemplate", "createCardTemplate", "updateCardTemplate", "deleteCardTemplate", "previewTemplateCards", "generateCardsFromTemplate"])
domain("settings", ["AppSettings", "AppInfo", "getSettings", "saveSettings", "getAppInfo"])
domain("practice", ["PracticeMode", "PracticeCard", "PracticeCardWindow", "PracticeSession", "PracticeWorkloadOverview", "StartPracticeInput", "StartCardPracticeInput", "PracticeCardWindowInput", "ReviewOutcome", "IntervalPreview", "ReviewPreviewSet", "SubmitReviewInput", "ReviewResult", "getPracticeWorkload", "startSelectedPractice", "startCardPractice", "getPracticeCardWindow", "markPracticeCardViewed", "markPracticeCardRevealed", "submitPracticeReview", "completePracticeSession"])
domain("scheduling", ["ScheduleAction", "RescheduleIntent", "ScheduleActionInput", "ScheduleActionResult", "applyScheduleAction"])
domain("backup", ["LibraryBackupResult", "LibraryRestoreResult", "exportLibraryBackup", "prepareLibraryRestore"])
domain("analytics", ["AnalyticsRange", "TagAnalyticsStatus", "AttributionSource", "AnalyticsQuery", "AnalyticsScope", "CurrentInventorySummary", "HistoricalReviewSummary", "DailyLimitUsage", "RatingDistribution", "TimelineBucket", "DeckAnalytics", "TagAnalytics", "RecentReview", "SessionAnalytics", "AttributionCoverage", "AnalyticsDeckOption", "AnalyticsTagOption", "AnalyticsDimensionOptions", "Analytics", "getAnalytics", "getRecentPracticeReviews"])
domain("generated-import", ["FlashcardImportResult", "importGeneratedFlashcards"])

ALIASES = {
    "common": "commonIpc",
    "assets": "assetsIpc",
    "decks": "decksIpc",
    "cards": "cardsIpc",
    "templates": "templatesIpc",
    "settings": "settingsIpc",
    "practice": "practiceIpc",
    "scheduling": "schedulingIpc",
    "backup": "backupIpc",
    "analytics": "analyticsIpc",
    "generated-import": "generatedImportIpc",
}


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    (ROOT / path).write_text(text, encoding="utf-8")


def rel_module(path: Path, domain_name: str) -> str:
    target = SRC / "ipc" / domain_name
    rel = os.path.relpath(target, path.parent).replace(os.sep, "/")
    return rel if rel.startswith(".") else f"./{rel}"


def scan_balanced(text: str, start: int, opening: str = "{", closing: str = "}") -> int:
    depth = 0
    quote: str | None = None
    escape = False
    index = start
    while index < len(text):
        char = text[index]
        if quote is not None:
            if escape:
                escape = False
            elif char == "\\":
                escape = True
            elif char == quote:
                quote = None
            index += 1
            continue
        if char in {'"', "'", "`"}:
            quote = char
            index += 1
            continue
        if char == opening:
            depth += 1
        elif char == closing:
            depth -= 1
            if depth == 0:
                return index
        index += 1
    raise SystemExit("unterminated balanced expression")


def split_members(body: str) -> list[str]:
    members: list[str] = []
    start = 0
    curly = square = paren = 0
    quote: str | None = None
    escape = False
    for index, char in enumerate(body):
        if quote is not None:
            if escape:
                escape = False
            elif char == "\\":
                escape = True
            elif char == quote:
                quote = None
            continue
        if char in {'"', "'", "`"}:
            quote = char
            continue
        if char == "{": curly += 1
        elif char == "}": curly -= 1
        elif char == "[": square += 1
        elif char == "]": square -= 1
        elif char == "(": paren += 1
        elif char == ")": paren -= 1
        elif char == "," and curly == square == paren == 0:
            value = body[start:index].strip()
            if value:
                members.append(value)
            start = index + 1
    value = body[start:].strip()
    if value:
        members.append(value)
    return members


def member_name(member: str) -> str:
    match = re.match(r'(?:["\']?)([A-Za-z_$][A-Za-z0-9_$]*)(?:["\']?)\s*:', member)
    if match:
        return match.group(1)
    match = re.match(r'([A-Za-z_$][A-Za-z0-9_$]*)\b', member)
    if match:
        return match.group(1)
    raise SystemExit(f"unable to identify mock member: {member[:100]!r}")


def rewrite_namespace_test(path: Path, text: str) -> str:
    match = re.search(r'import \* as ipc from ["\']([^"\']*ipc)["\'];', text)
    if not match:
        return text
    old_module = match.group(1)
    symbols = sorted(set(re.findall(r'\bipc\.([A-Za-z_$][A-Za-z0-9_$]*)', text)))
    grouped: dict[str, list[str]] = {}
    for symbol in symbols:
        domain_name = DOMAIN_SYMBOLS.get(symbol)
        if domain_name is None:
            if symbol in {"getCardTypeInfo", "CardTypeInfo"}:
                continue
            raise SystemExit(f"{path.relative_to(ROOT)}: unmapped ipc namespace symbol {symbol}")
        grouped.setdefault(domain_name, []).append(symbol)
    imports = "\n".join(
        f'import * as {ALIASES[name]} from "{rel_module(path, name)}";'
        for name in sorted(grouped)
    )
    text = text[:match.start()] + imports + text[match.end():]
    for domain_name, domain_symbols in grouped.items():
        alias = ALIASES[domain_name]
        for symbol in sorted(domain_symbols, key=len, reverse=True):
            text = re.sub(rf'\bipc\.{re.escape(symbol)}\b', f'{alias}.{symbol}', text)

    # Split each old aggregate mock into the real domain modules used by the
    # production screen. Vitest may hoist each independent factory safely.
    while True:
        marker = f'vi.mock("{old_module}", () => ({{'
        start = text.find(marker)
        if start < 0:
            marker = f"vi.mock('{old_module}', () => ({{"
            start = text.find(marker)
        if start < 0:
            break
        body_start = start + len(marker) - 1
        body_end = scan_balanced(text, body_start)
        close = text.find('));', body_end)
        if close < 0:
            raise SystemExit(f"{path.relative_to(ROOT)}: aggregate mock closing not found")
        members = split_members(text[body_start + 1:body_end])
        mock_groups: dict[str, list[str]] = {}
        for member in members:
            name = member_name(member)
            if name in {"getCardTypeInfo", "CardTypeInfo"}:
                continue
            domain_name = DOMAIN_SYMBOLS.get(name)
            if domain_name is None:
                raise SystemExit(f"{path.relative_to(ROOT)}: unmapped mock member {name}")
            mock_groups.setdefault(domain_name, []).append(member)
        mocks = []
        for domain_name in sorted(mock_groups):
            module = rel_module(path, domain_name)
            body = ",\n  ".join(mock_groups[domain_name])
            mocks.append(f'vi.mock("{module}", () => ({{\n  {body},\n}}));')
        text = text[:start] + "\n".join(mocks) + text[close + 3:]
    return text


# Rewrite aggregate test imports/mocks plus old standalone gateway specifiers.
for path in SRC.rglob("*.test.ts*"):
    text = path.read_text(encoding="utf-8")
    text = rewrite_namespace_test(path, text)
    replacements = {
        "../analytics-ipc": rel_module(path, "analytics"),
        "./analytics-ipc": rel_module(path, "analytics"),
        "../generatedImport": rel_module(path, "generated-import"),
        "./generatedImport": rel_module(path, "generated-import"),
    }
    for old, new in replacements.items():
        text = text.replace(f'"{old}"', f'"{new}"').replace(f"'{old}'", f"'{new}'")
    path.write_text(text, encoding="utf-8")

# Tests moved under src/ipc/ need their factory path corrected.
path = "src/ipc/domain-contracts.test.ts"
text = read(path).replace('from "./test/factories"', 'from "../test/factories"')
text = re.sub(r'\n\s*await expectInvocation\(\(\) => getCardTypeInfo\(noteId\)[\s\S]*?\n\s*\);', '', text, count=1)
write(path, text)

# A few tests construct PracticeCard literals directly rather than through the
# shared factory.
for path_obj in list((SRC / "screens" / "practice").glob("*.test.tsx")) + list((SRC / "screens").glob("PracticeScreen*.test.tsx")):
    text = path_obj.read_text(encoding="utf-8")
    text = re.sub(r'(^\s*)card:', r'\1presentation:', text, flags=re.MULTILINE)
    path_obj.write_text(text, encoding="utf-8")

# CardPresentation uses cardId, not the old generic id.
for path_obj in [SRC / "screens" / "PracticeScreen.tsx", *(SRC / "screens" / "practice").glob("*.ts*")]:
    if not path_obj.exists():
        continue
    text = path_obj.read_text(encoding="utf-8")
    text = re.sub(r'\.presentation\.id\b', '.presentation.cardId', text)
    path_obj.write_text(text, encoding="utf-8")

# RequestLane suppresses obsolete errors as well as obsolete successes. This is
# essential because Tauri work itself is not cancellable.
path = "src/controllers/requestLane.ts"
text = read(path)
old = '''  const token = lane.begin();
  const value = await operation();
  return lane.isCurrent(token) ? value : undefined;'''
new = '''  const token = lane.begin();
  try {
    const value = await operation();
    return lane.isCurrent(token) ? value : undefined;
  } catch (error: unknown) {
    if (!lane.isCurrent(token)) return undefined;
    throw error;
  }'''
if text.count(old) != 1:
    raise SystemExit("requestLane latestResult body drifted")
write(path, text.replace(old, new, 1))

# Cards controller also owns bounded selection reads so a stale "select all" or
# practice-all resolution cannot overwrite a newer filter scope.
path = "src/controllers/cardsController.ts"
text = read(path)
text = text.replace('  listTags,\n  searchCards,', '  listMatchingCards,\n  listTags,\n  searchCards,')
text = text.replace('  readonly #note = new RequestLane();', '  readonly #note = new RequestLane();\n  readonly #matching = new RequestLane();')
text = text.replace('''  note(noteId: string): Promise<CardNote | undefined> {
    return latestResult(this.#note, () => getCardNote(noteId));
  }
''', '''  note(noteId: string): Promise<CardNote | undefined> {
    return latestResult(this.#note, () => getCardNote(noteId));
  }

  matching(input: CardSearchInput): Promise<SelectionCard[] | undefined> {
    return latestResult(this.#matching, () => listMatchingCards(input));
  }
''')
text = text.replace('  type CardSearchPage,\n  type TagSummary,', '  type CardSearchPage,\n  type SelectionCard,\n  type TagSummary,')
text = text.replace('    this.#note.invalidate();', '    this.#note.invalidate();\n    this.#matching.invalidate();')
write(path, text)

# Wire Search/Filter reads through CardsController and preserve the explicit
# committed-mutation/reconciliation distinction.
path = "src/screens/CardsScreen.tsx"
text = read(path)
text = text.replace('import type { ChangeEvent, MutableRefObject } from "react";\n', 'import type { ChangeEvent, MutableRefObject } from "react";\n\nimport { CardsController } from "../controllers/cardsController";\nimport { reconcileCommitted } from "../controllers/reconciliation";\n')
for symbol in ['  getCardNote,\n', '  listMatchingCards,\n', '  listTags,\n', '  searchCards,\n']:
    text = text.replace(symbol, '')
text = text.replace('import { MoveCopyModal } from "./cards/MoveCopyModal";', 'import { MoveCopyModal, type TransferableCard } from "./cards/MoveCopyModal";')
text = text.replace('  const [moveCopyCards, setMoveCopyCards] = useState<SelectionCard[]>([]);', '  const [moveCopyCards, setMoveCopyCards] = useState<TransferableCard[]>([]);')
text = text.replace('  const openCardRequestSequence = useRef(0);\n', '')
text = text.replace('  const requestSequence = useRef(0);\n', '')
insert = '''  const controllerRef = useRef<CardsController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new CardsController();
  const controller = controllerRef.current;
'''
needle = '  const pendingScheduleMutationRef = useRef<PendingScheduleMutation | null>(null);\n'
if text.count(needle) != 1: raise SystemExit('CardsScreen controller insertion point drifted')
text = text.replace(needle, needle + insert, 1)
text = text.replace('''  const loadTags = useCallback(async (): Promise<void> => {
    try {
      const loaded = await listTags([...deckIds]);
      setTags(loaded);''', '''  const loadTags = useCallback(async (): Promise<void> => {
    try {
      const loaded = await controller.tags([...deckIds]);
      if (!loaded) return;
      setTags(loaded);''')
text = text.replace('  }, [deckIds]);', '  }, [controller, deckIds]);', 1)
old_load = re.compile(r'''  const loadCards = useCallback\(\n    async \(offset = 0, append = false\): Promise<void> => \{[\s\S]*?\n    \},\n    \[searchInput\],\n  \);''')
new_load = '''  const loadCards = useCallback(
    async (offset = 0, append = false): Promise<void> => {
      if (append) setIsLoadingMore(true);
      else {
        setIsLoading(true);
        setIsLoadingMore(false);
        setPage({ cards: [], totalCount: 0 });
      }
      setErrorMessage(null);
      try {
        const result = await controller.search({ ...searchInput, offset });
        if (!result) return;
        setPage((current) => ({
          totalCount: result.totalCount,
          cards: append ? [...current.cards, ...result.cards] : result.cards,
        }));
        setIsLoading(false);
        setIsLoadingMore(false);
      } catch (error: unknown) {
        setErrorMessage(getErrorMessage(error));
        setIsLoading(false);
        setIsLoadingMore(false);
      }
    },
    [controller, searchInput],
  );'''
text, count = old_load.subn(new_load, text, count=1)
if count != 1: raise SystemExit('CardsScreen loadCards block drifted')
text = text.replace('''  async function reconcileAfterCommit(subject: string): Promise<void> {
    try {
      await refreshAfterMutation();
    } catch (error: unknown) {
      showToast(
        "info",
        `${subject} was saved, but the current view could not refresh: ${getErrorMessage(error)}`,
      );
    }
  }''', '''  async function reconcileAfterCommit(subject: string): Promise<void> {
    const outcome = await reconcileCommitted(undefined, refreshAfterMutation);
    if (outcome.reconciliationError) {
      showToast(
        "info",
        `${subject} was saved, but the current view could not refresh: ${outcome.reconciliationError}`,
      );
    }
  }''')
old_open = re.compile(r'''  async function openCard\(noteId: string\): Promise<void> \{[\s\S]*?\n  \}\n\n  function toggleSelected''')
new_open = '''  async function openCard(noteId: string): Promise<void> {
    setIsOpeningCard(true);
    setErrorMessage(null);
    try {
      const card = await controller.note(noteId);
      if (!card) return;
      setEditingCard(card);
      setIsOpeningCard(false);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
      setIsOpeningCard(false);
    }
  }

  function toggleSelected'''
text, count = old_open.subn(new_open, text, count=1)
if count != 1: raise SystemExit('CardsScreen openCard block drifted')
text = text.replace('const matches = await listMatchingCards(searchInput);\n      setSelectedCards', 'const matches = await controller.matching(searchInput);\n      if (!matches) return;\n      setSelectedCards')
text = text.replace('const matches = await listMatchingCards(searchInput);\n      setPracticeSourceLabel', 'const matches = await controller.matching(searchInput);\n      if (!matches) return;\n      setPracticeSourceLabel')
text = text.replace('  function beginMoveCopy(cards: SelectionCard[]): void {', '  function beginMoveCopy(cards: TransferableCard[]): void {')
text = text.replace('                      cardId: editingCard.id,\n', '')
text = text.replace('"CardPresentation updated."', '"Flashcard updated."').replace('"CardPresentation saved."', '"Flashcard saved."')
# Cleanup invalidates controller lanes on unmount.
anchor = '  useEffect(() => {\n    setDeckIds((current) => {'
cleanup = '  useEffect(() => () => controller.invalidate(), [controller]);\n\n'
if anchor not in text: raise SystemExit('CardsScreen cleanup anchor drifted')
text = text.replace(anchor, cleanup + anchor, 1)
write(path, text)

# Decks reads use the controller; asset/import operations remain ordinary
# mutations and therefore do not use stale-read cancellation.
path = "src/screens/DecksScreen.tsx"
text = read(path)
text = text.replace('import type { FormEvent } from "react";\n', 'import type { FormEvent } from "react";\n\nimport { DecksController } from "../controllers/decksController";\n')
for symbol in ['  getDeck,\n', '  getCardNote,\n', '  listDeckCardSummaries,\n']:
    text = text.replace(symbol, '')
text = text.replace('  const cardRequestSequence = useRef(0);\n', '')
needle = '  const [cardSort, setCardSort] = useState<"updated_desc" | "question_asc">("updated_desc");\n'
insert = '''  const controllerRef = useRef<DecksController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new DecksController();
  const controller = controllerRef.current;
'''
if text.count(needle) != 1: raise SystemExit('DecksScreen controller insertion drifted')
text = text.replace(needle, needle + insert, 1)
# Detail effect.
text = text.replace('''    const requestId = cardRequestSequence.current + 1;
    cardRequestSequence.current = requestId;
    const timer = window.setTimeout(() => {''', '''    const timer = window.setTimeout(() => {''')
text = text.replace('void listDeckCardSummaries({', 'void controller.cards({', 1)
text = text.replace('''        .then((page) => {
          if (cardRequestSequence.current !== requestId) return;
          setCards(page.cards);''', '''        .then((page) => {
          if (!page) return;
          setCards(page.cards);''', 1)
text = text.replace('''        .catch((error: unknown) => {
          if (cardRequestSequence.current !== requestId) return;''', '''        .catch((error: unknown) => {''', 1)
text = text.replace('''        .finally(() => {
          if (cardRequestSequence.current === requestId) setIsLoadingCards(false);
        });''', '''        .finally(() => setIsLoadingCards(false));''', 1)
# Add controller dependency to detail effect dependency array.
text = text.replace('  }, [cardSort, search, selectedDeckId, view]);', '  }, [cardSort, controller, search, selectedDeckId, view]);')
text = text.replace('const deck = await getDeck(deckId);', 'const deck = await controller.deck(deckId);\n      if (!deck) return;', 1)
# refreshSelectedDeckAfterMutation uses current controller reads.
text = text.replace('const deck = await getDeck(deckId);\n      setSelectedDeck(deck);\n      const requestId = cardRequestSequence.current + 1;\n      cardRequestSequence.current = requestId;\n      const page = await listDeckCardSummaries({', 'const deck = await controller.deck(deckId);\n      if (!deck) return;\n      setSelectedDeck(deck);\n      const page = await controller.cards({', 1)
text = text.replace('''      if (cardRequestSequence.current === requestId) {
        setCards(page.cards);
        setCardTotalCount(page.totalCount);
        setHasMoreCards(page.hasMore);
      }''', '''      if (page) {
        setCards(page.cards);
        setCardTotalCount(page.totalCount);
        setHasMoreCards(page.hasMore);
      }''', 1)
# loadMore cards.
text = text.replace('''    const requestId = cardRequestSequence.current + 1;
    cardRequestSequence.current = requestId;
    setIsLoadingCards(true);
    try {
      const page = await listDeckCardSummaries({''', '''    setIsLoadingCards(true);
    try {
      const page = await controller.cards({''', 1)
text = text.replace('''      if (cardRequestSequence.current !== requestId) return;
      setCards((current) => [...current, ...page.cards]);''', '''      if (!page) return;
      setCards((current) => [...current, ...page.cards]);''', 1)
text = text.replace('''    } catch (error: unknown) {
      if (cardRequestSequence.current === requestId) showToast("error", getErrorMessage(error));
    } finally {
      if (cardRequestSequence.current === requestId) setIsLoadingCards(false);
    }''', '''    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      setIsLoadingCards(false);
    }''', 1)
text = text.replace('const card = await getCardNote(summary.noteId);', 'const card = await controller.note(summary.noteId);\n      if (!card) return;', 1)
# Cleanup.
anchor = '  const selectedDeckId = selectedDeck?.id ?? null;\n'
text = text.replace(anchor, anchor + '\n  useEffect(() => () => controller.invalidate(), [controller]);\n', 1)
write(path, text)

# Card Editor: use one controller for all mutable-scope reads. The card-type
# follow-up load is already removed by the CardNote DTO migration.
path = "src/screens/CardEditorScreen.tsx"
text = read(path)
text = text.replace('import { useEffect, useMemo, useRef, useState } from "react";\n', 'import { useEffect, useMemo, useRef, useState } from "react";\n\nimport { CardEditorController } from "../controllers/cardEditorController";\n')
for symbol in ['  getCardTemplate,\n', '  listCardTemplateSummaries,\n', '  listDecks,\n', '  listTags,\n', '  previewTemplateCards,\n']:
    text = text.replace(symbol, '')
text = text.replace('  const templateRequestSequence = useRef(0);\n', '')
text = text.replace('  const [selectedTemplateKey, setSelectedTemplateKey] = useState(card?.cardType ?? "basic");', '  const [selectedTemplateKey, setSelectedTemplateKey] = useState<string>(card?.cardType ?? "basic");')
text = text.replace('    !isLoadingType &&\n', '')
text = text.replace('isLoadingType || ', '').replace(' || isLoadingType', '')
needle = '  const [dirty, setDirty] = useState(false);\n'
insert = '''  const controllerRef = useRef<CardEditorController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new CardEditorController();
  const controller = controllerRef.current;
'''
if text.count(needle) != 1: raise SystemExit('CardEditor controller insertion drifted')
text = text.replace(needle, needle + insert, 1)
# provided decks effect.
text = text.replace('void listDecks()\n      .then((decks) => {\n        if (current) setLoadedDecks(decks);', 'void controller.decks()\n      .then((decks) => {\n        if (current && decks) setLoadedDecks(decks);')
text = text.replace('  }, [providedDecks]);', '  }, [controller, providedDecks]);', 1)
# template summaries.
text = text.replace('void listCardTemplateSummaries()\n      .then((loaded) => {\n        if (current) setTemplateSummaries(loaded);', 'void controller.templateSummaries()\n      .then((loaded) => {\n        if (current && loaded) setTemplateSummaries(loaded);')
text = text.replace('  }, []);', '  }, [controller]);', 1)
# custom template selection effect; replace sequence logic block with controller.
pattern = re.compile(r'''  useEffect\(\(\) => \{\n    if \(selectedTemplateKey === "basic" \|\| selectedTemplateKey === "cloze"\) \{[\s\S]*?\n  \}, \[selectedTemplateKey\]\);''')
replacement = '''  useEffect(() => {
    if (selectedTemplateKey === "basic" || selectedTemplateKey === "cloze") {
      controller.invalidateTemplate();
      setSelectedCustomTemplate(null);
      setIsLoadingTemplate(false);
      return;
    }
    setSelectedCustomTemplate(null);
    setTemplateValues([]);
    setTemplatePreviews([]);
    setIsLoadingTemplate(true);
    void controller
      .template(selectedTemplateKey)
      .then((template) => {
        if (template) setSelectedCustomTemplate(template);
        setIsLoadingTemplate(false);
      })
      .catch((error: unknown) => {
        setErrorMessage(getErrorMessage(error));
        setIsLoadingTemplate(false);
      });
  }, [controller, selectedTemplateKey]);'''
text, count = pattern.subn(replacement, text, count=1)
if count != 1: raise SystemExit('CardEditor template load effect drifted')
# tags.
text = text.replace('void listTags()\n      .then(setTagSuggestions)', 'void controller.tags()\n      .then((loaded) => { if (loaded) setTagSuggestions(loaded); })')
# preview.
text = text.replace('''      setTemplatePreviews(
        await previewTemplateCards({
          templateId: selectedCustomTemplate.id,
          templateRevision: selectedCustomTemplate.revision,
          values: templateValues,
        }),
      );''', '''      const preview = await controller.preview({
        templateId: selectedCustomTemplate.id,
        templateRevision: selectedCustomTemplate.revision,
        values: templateValues,
      });
      if (preview) setTemplatePreviews(preview);''')
# cleanup controller lanes near dirty effect.
anchor = '  useEffect(() => {\n    onDirtyChange(dirty);'
text = text.replace(anchor, '  useEffect(() => () => controller.invalidate(), [controller]);\n\n' + anchor, 1)
write(path, text)

# Templates library/open reads use the controller. Existing openRequestSequence
# remains for the asset-draft lifecycle, which is a resource mutation.
path = "src/screens/TemplatesScreen.tsx"
text = read(path)
text = text.replace('import type { ShowToast } from "../components/Toast";\n', 'import type { ShowToast } from "../components/Toast";\nimport { TemplatesController } from "../controllers/templatesController";\n')
text = text.replace('  getCardTemplate,\n', '').replace('  listCardTemplateSummaries,\n', '')
needle = '  const savingRef = useRef(false);\n'
insert = '''  const controllerRef = useRef<TemplatesController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new TemplatesController();
  const controller = controllerRef.current;
'''
if text.count(needle) != 1: raise SystemExit('Templates controller insertion drifted')
text = text.replace(needle, needle + insert, 1)
text = text.replace('setTemplates(await listCardTemplateSummaries());', 'const loaded = await controller.list();\n      if (loaded) setTemplates(loaded);')
text = text.replace('const template = await getCardTemplate(summary.id);', 'const template = await controller.template(summary.id);\n      if (!template) return;')
anchor = '  useEffect(() => {\n    void loadTemplates();\n  }, []);'
text = text.replace(anchor, '  useEffect(() => {\n    void loadTemplates();\n    return () => controller.invalidate();\n  }, [controller]);', 1)
write(path, text)

# Practice read lanes. Mutation retry refs remain local and durable-operation
# semantics stay unchanged.
path = "src/screens/PracticeScreen.tsx"
text = read(path)
text = text.replace('import type { PracticeLaunchRequest } from "../App";\n', 'import type { PracticeLaunchRequest } from "../App";\nimport { PracticeController } from "../controllers/practiceController";\n')
text = text.replace('  getRecentPracticeReviews,\n', '')
for symbol in ['  getPracticeCardWindow,\n', '  getPracticeWorkload,\n']:
    text = text.replace(symbol, '')
text = text.replace('  const workloadRequestSequence = useRef(0);\n', '').replace('  const windowRequestSequence = useRef(0);\n', '')
needle = '  const pendingReviewRef = useRef<PendingReviewOperation | null>(null);\n'
insert = '''  const controllerRef = useRef<PracticeController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new PracticeController();
  const controller = controllerRef.current;
'''
if text.count(needle) != 1: raise SystemExit('Practice controller insertion drifted')
text = text.replace(needle, needle + insert, 1)
text = text.replace('setRecentDecks(summarizeRecentDecks(await getRecentPracticeReviews()));', 'const reviews = await controller.recentReviews();\n      if (reviews) setRecentDecks(summarizeRecentDecks(reviews));')
text = text.replace('  }, []);', '  }, [controller]);', 1)
# workload function whole block.
pattern = re.compile(r'''  const loadWorkload = useCallback\(async \(deckIds: string\[], tagIds: string\[]\): Promise<void> => \{[\s\S]*?\n  \}, \[]\);''')
replacement = '''  const loadWorkload = useCallback(async (deckIds: string[], tagIds: string[]): Promise<void> => {
    if (deckIds.length === 0 && tagIds.length === 0) {
      controller.invalidateWorkload();
      setWorkload(null);
      setIsLoadingWorkload(false);
      return;
    }
    setIsLoadingWorkload(true);
    try {
      const loaded = await controller.workload(deckIds, tagIds);
      if (!loaded) return;
      setWorkload(loaded);
      setErrorMessage(null);
      setIsLoadingWorkload(false);
    } catch (error: unknown) {
      setWorkload(null);
      setErrorMessage(getErrorMessage(error));
      setIsLoadingWorkload(false);
    }
  }, [controller]);'''
text, count = pattern.subn(replacement, text, count=1)
if count != 1: raise SystemExit('Practice workload block drifted')
text = text.replace('    windowRequestSequence.current += 1;\n', '')
# next-window request sequence bits.
text = text.replace('''    const requestId = windowRequestSequence.current + 1;
    windowRequestSequence.current = requestId;
    try {
      const window = await getPracticeCardWindow({''', '''    try {
      const window = await controller.window({''')
text = text.replace('''      if (windowRequestSequence.current !== requestId) return sessionCards.length;
      setSessionCards''', '''      if (!window) return sessionCards.length;
      setSessionCards''')
text = text.replace('''    } catch (error: unknown) {
      if (windowRequestSequence.current === requestId) setErrorMessage(getErrorMessage(error));
      return sessionCards.length;
    } finally {
      if (windowRequestSequence.current === requestId) {
        loadingWindowRef.current = false;
        setIsLoadingWindow(false);
      }
    }''', '''    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
      return sessionCards.length;
    } finally {
      loadingWindowRef.current = false;
      setIsLoadingWindow(false);
    }''')
text = text.replace('  }, [session, sessionCards.length]);', '  }, [controller, session, sessionCards.length]);')
# presentation ids.
text = re.sub(r'\.presentation\.id\b', '.presentation.cardId', text)
# Cleanup.
anchor = '  const current = sessionCards[position] ?? null;\n'
text = text.replace(anchor, anchor + '\n  useEffect(() => () => controller.invalidate(), [controller]);\n', 1)
write(path, text)

# Analytics has one controller lane and no longer owns an ad-hoc sequence ref.
path = "src/screens/AnalyticsScreen.tsx"
text = read(path)
text = text.replace('import type { AnalyticsLaunchRequest } from "../App";\n', 'import type { AnalyticsLaunchRequest } from "../App";\nimport { AnalyticsController } from "../controllers/analyticsController";\n')
text = text.replace('  getAnalytics,\n', '')
text = text.replace('  const requestSequence = useRef(0);\n', '')
needle = '  const handledLaunchRef = useRef<number | null>(null);\n'
insert = '''  const controllerRef = useRef<AnalyticsController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new AnalyticsController();
  const controller = controllerRef.current;
'''
if text.count(needle) != 1: raise SystemExit('Analytics controller insertion drifted')
text = text.replace(needle, needle + insert, 1)
pattern = re.compile(r'''  const load = useCallback\(async \(\): Promise<void> => \{[\s\S]*?\n  \}, \[deckIds, highlightedSessionId, range, showToast, tagIds\]\);''')
replacement = '''  const load = useCallback(async (): Promise<void> => {
    setIsLoading(true);
    setErrorMessage(null);
    try {
      const result = await controller.load({
        range,
        deckIds,
        tagIds,
        highlightedSessionId,
      });
      if (!result) return;
      setAnalytics(result);
      const availableDeckIds = new Set(result.dimensionOptions.decks.map((deck) => deck.id));
      const availableTagIds = new Set(result.dimensionOptions.tags.map((tag) => tag.id));
      setSelectedDeckIds((current) => filteredSet(current, availableDeckIds));
      setSelectedTagIds((current) => filteredSet(current, availableTagIds));
      setIsLoading(false);
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
      setIsLoading(false);
    }
  }, [controller, deckIds, highlightedSessionId, range, showToast, tagIds]);'''
text, count = pattern.subn(replacement, text, count=1)
if count != 1: raise SystemExit('Analytics load block drifted')
anchor = '  useEffect(() => {\n    void load();\n  }, [load]);'
text = text.replace(anchor, '  useEffect(() => {\n    void load();\n    return () => controller.invalidate();\n  }, [controller, load]);', 1)
write(path, text)

# Remaining obvious CardNote/Presentation compiler residues.
path = "src/screens/CardEditorScreen.tsx"
text = read(path).replace('isLoadingType || ', '').replace(' || isLoadingType', '')
write(path, text)

print("Applied Stage 3.12 controller wiring and frontend contract fixes.")

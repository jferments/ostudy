#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    (ROOT / path).write_text(text, encoding="utf-8")


def require_replace(path: str, text: str, old: str, new: str, count: int = 1) -> str:
    actual = text.count(old)
    if actual != count:
        raise SystemExit(f"{path}: expected {count} occurrences, found {actual}: {old[:90]!r}")
    return text.replace(old, new, count)


# Search/Filter: transport mutations go through CardsController, and the large
# filter tree becomes a pure presentational component.
path = "src/screens/CardsScreen.tsx"
text = read(path)
text = require_replace(path, text, 'import type { ChangeEvent, MutableRefObject } from "react";', 'import type { MutableRefObject } from "react";')
text = require_replace(path, text, '  applyScheduleAction,\n', '')
text = require_replace(
    path,
    text,
    'import { BulkEntryModal } from "./cards/BulkEntryModal";',
    'import { BulkEntryModal } from "./cards/BulkEntryModal";\nimport { CardFilters } from "./cards/CardFilters";',
)
text = text.replace('await applyScheduleAction({', 'await controller.schedule({')
start = text.index('      <section className="card-library-toolbar" aria-label="Card filters">')
end = text.index('      </section>', start) + len('      </section>')
filters = '''      <CardFilters
        activeDecks={activeDecks}
        deckIds={deckIds}
        query={query}
        availabilityFilter={availabilityFilter}
        statusFilter={statusFilter}
        reviewedFilter={reviewedFilter}
        sort={sort}
        tags={tags}
        tagIds={tagIds}
        onDeckIdsChange={setDeckIds}
        onQueryChange={(value) => {
          setQuery(value);
          if (value.trim() !== "" && sort === "updated_desc") setSort("relevance");
        }}
        onAvailabilityFilterChange={setAvailabilityFilter}
        onStatusFilterChange={setStatusFilter}
        onReviewedFilterChange={setReviewedFilter}
        onSortChange={setSort}
        onTagIdsChange={setTagIds}
      />'''
text = text[:start] + filters + text[end:]
text, count = re.subn(
    r'\nfunction deckFilterSummary\(deckIds: Set<string>, decks: DeckSummary\[]\): string \{[\s\S]*?\n\}\n\nfunction summarize',
    '\nfunction summarize',
    text,
    count=1,
)
if count != 1:
    raise SystemExit("CardsScreen.tsx: deckFilterSummary helper drifted")
write(path, text)

# Analytics: one pure filter component owns range/scope presentation; controller
# continues to own the latest-request lane and data orchestration.
path = "src/screens/AnalyticsScreen.tsx"
text = read(path)
text = require_replace(
    path,
    text,
    'import { ProgressChart } from "./analytics/ProgressChart";',
    'import { AnalyticsFilters } from "./analytics/AnalyticsFilters";\nimport { ProgressChart } from "./analytics/ProgressChart";',
)
for block in [
    '''  const selectedDeckNames = deckIds
    .map((id) => deckOptions.find((deck) => deck.id === id)?.name ?? decks.find((deck) => deck.id === id)?.name)
    .filter((name): name is string => Boolean(name));
''',
    '''  const selectedTagNames = tagIds
    .map((id) => tagOptions.find((tag) => tag.id === id)?.name)
    .filter((name): name is string => Boolean(name));
''',
]:
    text = require_replace(path, text, block, '')
start = text.index('      <section className="analytics-filter-panel" aria-label="Analytics filters">')
end = text.index('      </section>', start) + len('      </section>')
filters = '''      <AnalyticsFilters
        range={range}
        selectedDeckIds={selectedDeckIds}
        selectedTagIds={selectedTagIds}
        highlightedSessionId={highlightedSessionId}
        deckOptions={deckOptions}
        tagOptions={tagOptions}
        decks={decks}
        onRangeChange={setRange}
        onDeckIdsChange={setSelectedDeckIds}
        onTagIdsChange={setSelectedTagIds}
        onClearHighlightedSession={() => setHighlightedSessionId(undefined)}
        onClearScope={() => {
          setSelectedDeckIds(new Set());
          setSelectedTagIds(new Set());
          setHighlightedSessionId(undefined);
        }}
      />'''
text = text[:start] + filters + text[end:]
text, count = re.subn(
    r'\ninterface FilterOption \{[\s\S]*?\n\}\n\ninterface MetricCardProps',
    '\ninterface MetricCardProps',
    text,
    count=1,
)
if count != 1:
    raise SystemExit("AnalyticsScreen.tsx: FilterChecklist block drifted")
write(path, text)

# Decks: all native mutations/import/export calls are controller operations.
path = "src/screens/DecksScreen.tsx"
text = read(path)
text = require_replace(path, text, 'import { importGeneratedFlashcards } from "../ipc/generated-import";\n', '')
for symbol in ['  deleteCard,\n', '  undoDeletion,\n']:
    text = require_replace(path, text, symbol, '')
for symbol in [
    '  createDeck,\n', '  deleteDeck,\n', '  exportDeck,\n', '  importDeck,\n',
    '  setDeckArchived,\n', '  updateDeck,\n',
]:
    text = require_replace(path, text, symbol, '')
replacements = {
    'await updateDeck({': 'await controller.update({',
    'await createDeck(input)': 'await controller.create(input)',
    'await setDeckArchived({': 'await controller.setArchived({',
    'await deleteCard(': 'await controller.deleteNote(',
    'await deleteDeck(': 'await controller.deleteDeck(',
    'await undoDeletion(': 'await controller.undo(',
    'await exportDeck(': 'await controller.export(',
    'await importDeck(': 'await controller.importStudydeck(',
    'await importGeneratedFlashcards(': 'await controller.importGenerated(',
}
for old, new in replacements.items():
    if old not in text:
        raise SystemExit(f"DecksScreen.tsx: missing mutation call {old}")
    text = text.replace(old, new)
write(path, text)

# Card Editor: asset lifecycle, note writes, duplicate checks, and template
# generation are controller-owned; React retains only editor interaction state.
path = "src/screens/CardEditorScreen.tsx"
text = read(path)
for symbol in ['  beginAssetDraft,\n', '  cancelAssetDraft,\n', '  syncAssetDraft,\n']:
    text = require_replace(path, text, symbol, '')
for symbol in ['  createCardNote,\n', '  findDuplicateCards,\n', '  updateCardNote,\n']:
    text = require_replace(path, text, symbol, '')
text = require_replace(path, text, '  generateCardsFromTemplate,\n', '')
text = text.replace('beginAssetDraft("card", card?.noteId)', 'controller.beginDraft(card?.noteId)')
text = text.replace('cancelAssetDraft(', 'controller.cancelDraft(')
text = text.replace('syncAssetDraft(', 'controller.syncDraft(')
text = text.replace('await createCardNote(', 'await controller.create(')
text = text.replace('await updateCardNote(', 'await controller.update(')
text = text.replace('await findDuplicateCards(', 'await controller.duplicates(')
text = text.replace('await generateCardsFromTemplate(', 'await controller.generate(')
write(path, text)

# Templates: asset draft and CRUD calls live behind TemplatesController.
path = "src/screens/TemplatesScreen.tsx"
text = read(path)
for symbol in ['  beginAssetDraft,\n', '  cancelAssetDraft,\n', '  syncAssetDraft,\n']:
    text = require_replace(path, text, symbol, '')
for symbol in ['  createCardTemplate,\n', '  deleteCardTemplate,\n', '  updateCardTemplate,\n']:
    text = require_replace(path, text, symbol, '')
text = text.replace('beginAssetDraft("template", ', 'controller.beginDraft(')
text = text.replace('cancelAssetDraft(', 'controller.cancelDraft(')
text = text.replace('syncAssetDraft(', 'controller.syncDraft(')
text = text.replace('await createCardTemplate(', 'await controller.create(')
text = text.replace('await updateCardTemplate(', 'await controller.update(')
text = text.replace('await deleteCardTemplate(', 'await controller.delete(')
write(path, text)

# Practice: durable operations are explicit controller methods; retry/idempotency
# identifiers remain in the screen state machine, not in stale-read lanes.
path = "src/screens/PracticeScreen.tsx"
text = read(path)
for symbol in [
    '  completePracticeSession,\n', '  markPracticeCardRevealed,\n', '  markPracticeCardViewed,\n',
    '  startCardPractice,\n', '  startSelectedPractice,\n', '  submitPracticeReview,\n',
]:
    text = require_replace(path, text, symbol, '')
text = require_replace(path, text, '  applyScheduleAction,\n', '')
text = text.replace('await startSelectedPractice({', 'await controller.startSelected({')
text = text.replace('await startCardPractice({', 'await controller.startCards({')
text = text.replace('markPracticeCardViewed(', 'controller.markViewed(')
text = text.replace('await markPracticeCardRevealed(', 'await controller.reveal(')
text = text.replace('await submitPracticeReview(', 'await controller.review(')
text = text.replace('await applyScheduleAction({', 'await controller.schedule({')
text = text.replace('await completePracticeSession(', 'await controller.complete(')
write(path, text)

print("Applied Stage 3.12 controller mutation routing and view extraction.")

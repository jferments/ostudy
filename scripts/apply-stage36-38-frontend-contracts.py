#!/usr/bin/env python3
"""Migrate TypeScript IPC contracts to bounded Stage 3.6-3.8 DTOs."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
path = ROOT / "src/ipc.ts"
content = path.read_text()

content = content.replace(
'''export interface Deck {
  id: string;
  name: string;
  description: string;
  cardCount: number;
  dueCount: number;
  createdAt: number;
  updatedAt: number;
  revision: number;
}''',
'''export interface Deck {
  id: string;
  name: string;
  description: string;
  sourceNoteCount: number;
  generatedCardCount: number;
  studyableDueCount: number;
  createdAt: number;
  updatedAt: number;
  archivedAt: number | null;
  revision: number;
}''',
1)

insert_after = '''export interface CardTemplate {
  id: string;
  name: string;
  description: string;
  fields: CardTemplateField[];
  rules: CardTemplateRule[];
  assets: Asset[];
  createdAt: number;
  updatedAt: number;
  revision: number;
}
'''
addition = '''
export interface CardTemplateSummary {
  id: string;
  name: string;
  description: string;
  fieldCount: number;
  ruleCount: number;
  assetCount: number;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface DeckCardSummary {
  noteId: string;
  representativeCardId: string;
  cardType: BuiltInCardType;
  cardLabel: string;
  questionText: string;
  answerText: string;
  generatedCardCount: number;
  studyableDueCount: number;
  lastReviewedAt: number | null;
  updatedAt: number;
  revision: number;
}

export interface DeckCardPageInput {
  deckId: string;
  query: string;
  sort: "updated_desc" | "question_asc" | "due_asc";
  limit: number;
  offset: number;
}

export interface DeckCardPage {
  cards: DeckCardSummary[];
  totalCount: number;
  limit: number;
  offset: number;
}
'''
if "export interface CardTemplateSummary" not in content:
    content = content.replace(insert_after, insert_after + addition, 1)

content = content.replace(
'''export interface TagSummary {
  id: string;
  name: string;
  cardCount: number;
}

export type DueFilter = "all" | "due" | "not_due";''',
'''export interface TagSummary {
  id: string;
  name: string;
  sourceNoteCount: number;
  generatedCardCount: number;
  studyableDueCount: number;
}

export type AvailabilityFilter = "all" | "ready_now" | "scheduled_later";''',
1)
content = content.replace(
'''export type ReviewedFilter =
  | "all"
  | "never"
  | "last_1_day"
  | "last_7_days"
  | "last_30_days"
  | "older_30_days";''',
'''export type ReviewedFilter =
  | "all"
  | "never"
  | "today"
  | "past_7_dates"
  | "past_30_dates"
  | "before_30_dates";''',
1)
content = content.replace("  dueFilter: DueFilter;", "  availabilityFilter: AvailabilityFilter;", 1)
content = content.replace(
'''export interface PracticeSession {
  id: string;
  cards: PracticeCard[];
  shuffle: boolean;
  mode: PracticeMode;
  startedAt: number;
}''',
'''export interface PracticeSession {
  id: string;
  cards: PracticeCard[];
  cardCount: number;
  hasMore: boolean;
  shuffle: boolean;
  mode: PracticeMode;
  startedAt: number;
}

export interface PracticeCardWindow {
  startPosition: number;
  cards: PracticeCard[];
  hasMore: boolean;
}''',
1)
content = content.replace(
'''export interface StartPracticeInput {
  operationId: string;
  deckIds: string[];
  shuffle: boolean;
  mode: "scheduled" | "extra";
}''',
'''export interface StartPracticeInput {
  operationId: string;
  deckIds: string[];
  tagIds: string[];
  shuffle: boolean;
  mode: "scheduled" | "extra";
}''',
1)
content = content.replace("  dueAt?: number;", "  dueDate?: string;", 1)
content = content.replace(
'''export interface ScheduleActionResult {
  affectedCount: number;
  action: ScheduleAction;
}''',
'''export interface ScheduleActionResult {
  affectedCount: number;
  requestedCount: number;
  changedCount: number;
  unchangedCount: number;
  rejectedCount: number;
  action: ScheduleAction;
}''',
1)

content = content.replace(
'''export async function listDecks(): Promise<Deck[]> {
  return invoke<Deck[]>("list_decks");
}
''',
'''export async function listDecks(): Promise<Deck[]> {
  return invoke<Deck[]>("list_decks");
}

export async function listArchivedDecks(): Promise<Deck[]> {
  return invoke<Deck[]>("list_archived_decks");
}
''',
1)
content = content.replace(
'''export async function updateDeck(input: UpdateDeckInput): Promise<Deck> {
  return invoke<Deck>("update_deck", { input });
}
''',
'''export async function updateDeck(input: UpdateDeckInput): Promise<Deck> {
  return invoke<Deck>("update_deck", { input });
}

export async function archiveDeck(deckId: string): Promise<Deck> {
  return invoke<Deck>("archive_deck", { deckId });
}

export async function unarchiveDeck(deckId: string): Promise<Deck> {
  return invoke<Deck>("unarchive_deck", { deckId });
}
''',
1)
content = content.replace(
'''export async function listCards(deckId: string): Promise<Flashcard[]> {
  return invoke<Flashcard[]>("list_cards", { deckId });
}
''',
'''export async function listDeckCards(input: DeckCardPageInput): Promise<DeckCardPage> {
  return invoke<DeckCardPage>("list_deck_cards", { input });
}
''',
1)
content = content.replace(
'''export async function listCardTemplates(): Promise<CardTemplate[]> {
  return invoke<CardTemplate[]>("list_card_templates");
}
''',
'''export async function listCardTemplateSummaries(): Promise<CardTemplateSummary[]> {
  return invoke<CardTemplateSummary[]>("list_card_template_summaries");
}
''',
1)
content = content.replace(
'''export async function getPracticeWorkload(deckIds: string[]): Promise<PracticeWorkloadOverview> {
  return invoke<PracticeWorkloadOverview>("get_practice_workload", { input: { deckIds } });
}''',
'''export async function getPracticeWorkload(
  deckIds: string[],
  tagIds: string[] = [],
): Promise<PracticeWorkloadOverview> {
  return invoke<PracticeWorkloadOverview>("get_practice_workload", {
    input: { deckIds, tagIds },
  });
}''',
1)
content = content.replace(
'''export async function startCardPractice(input: StartCardPracticeInput): Promise<PracticeSession> {
  return invoke<PracticeSession>("start_card_practice", { input });
}
''',
'''export async function startCardPractice(input: StartCardPracticeInput): Promise<PracticeSession> {
  return invoke<PracticeSession>("start_card_practice", { input });
}

export async function getPracticeCardWindow(
  sessionId: string,
  startPosition: number,
  limit = 20,
): Promise<PracticeCardWindow> {
  return invoke<PracticeCardWindow>("get_practice_card_window", {
    input: { sessionId, startPosition, limit },
  });
}
''',
1)

path.write_text(content)
print("Migrated frontend IPC contracts.")

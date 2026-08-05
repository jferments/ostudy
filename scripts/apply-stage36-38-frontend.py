#!/usr/bin/env python3
"""Migrate frontend contracts and major screens to Stage 3.6–3.8 projections."""
from __future__ import annotations
import re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def read(p): return (ROOT/p).read_text(encoding='utf-8')
def write(p,c): (ROOT/p).write_text(c,encoding='utf-8')

# Typed native IPC contracts.
ipc=read('src/ipc.ts')
ipc=ipc.replace('''export interface Deck {
  id: string;
  name: string;
  description: string;
  cardCount: number;
  dueCount: number;
  createdAt: number;
  updatedAt: number;
  revision: number;
}''','''export interface Deck {
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
}''')
insert='''
export interface DeckCardSummary {
  noteId: string;
  primaryCardId: string;
  questionExcerpt: string;
  answerExcerpt: string;
  generatedCardCount: number;
  imageCount: number;
  updatedAt: number;
}

export interface DeckCardPage {
  cards: DeckCardSummary[];
  totalCount: number;
}

export interface CardTemplateSummary {
  id: string;
  name: string;
  description: string;
  fieldCount: number;
  generatedCardRuleCount: number;
  imageCount: number;
  updatedAt: number;
  revision: number;
}
'''
if 'export interface DeckCardSummary' not in ipc:
    ipc=ipc.replace('export interface CreateDeckInput {',insert+'\nexport interface CreateDeckInput {',1)
ipc=ipc.replace('''export interface PracticeSession {
  id: string;
  cards: PracticeCard[];
  shuffle: boolean;
  mode: PracticeMode;
  startedAt: number;
}''','''export interface PracticeSession {
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
}''')
ipc=ipc.replace('''export interface StartPracticeInput {
  operationId: string;
  deckIds: string[];
  shuffle: boolean;
  mode: "scheduled" | "extra";
}''','''export interface StartPracticeInput {
  operationId: string;
  deckIds: string[];
  tagIds: string[];
  shuffle: boolean;
  mode: "scheduled" | "extra";
}''')
ipc=ipc.replace('''export interface ScheduleActionInput {
  mutationId: string;
  cardIds: string[];
  action: ScheduleAction;
  dueAt?: number;
  sessionId?: string;
}''','''export type RescheduleIntent =
  | { kind: "dueNow" }
  | { kind: "tomorrow" }
  | { kind: "calendarDays"; days: number }
  | { kind: "calendarDate"; date: string };

export interface ScheduleActionInput {
  mutationId: string;
  cardIds: string[];
  action: ScheduleAction;
  reschedule?: RescheduleIntent;
  sessionId?: string;
}''')
ipc=ipc.replace('''export async function listCards(deckId: string): Promise<Flashcard[]> {
  return invoke<Flashcard[]>("list_cards", { deckId });
}''','''export async function listDeckCards(
  deckId: string,
  query = "",
  limit = 50,
  offset = 0,
): Promise<DeckCardPage> {
  return invoke<DeckCardPage>("list_deck_cards", { input: { deckId, query, limit, offset } });
}''')
if 'listArchivedDecks' not in ipc:
    ipc=ipc.replace('''export async function listDecks(): Promise<Deck[]> {
  return invoke<Deck[]>("list_decks");
}''','''export async function listDecks(): Promise<Deck[]> {
  return invoke<Deck[]>("list_decks");
}

export async function listArchivedDecks(): Promise<Deck[]> {
  return invoke<Deck[]>("list_archived_decks");
}

export async function archiveDeck(deckId: string): Promise<Deck> {
  return invoke<Deck>("archive_deck", { deckId });
}

export async function unarchiveDeck(deckId: string): Promise<Deck> {
  return invoke<Deck>("unarchive_deck", { deckId });
}''')
ipc=ipc.replace('''export async function listCardTemplates(): Promise<CardTemplate[]> {
  return invoke<CardTemplate[]>("list_card_templates");
}''','''export async function listCardTemplateSummaries(): Promise<CardTemplateSummary[]> {
  return invoke<CardTemplateSummary[]>("list_card_template_summaries");
}''')
ipc=ipc.replace('''export async function getPracticeWorkload(deckIds: string[]): Promise<PracticeWorkloadOverview> {
  return invoke<PracticeWorkloadOverview>("get_practice_workload", { input: { deckIds } });
}''','''export async function getPracticeWorkload(
  deckIds: string[],
  tagIds: string[] = [],
): Promise<PracticeWorkloadOverview> {
  return invoke<PracticeWorkloadOverview>("get_practice_workload", { input: { deckIds, tagIds } });
}''')
if 'getPracticeCardWindow' not in ipc:
    ipc=ipc.replace('''export async function startCardPractice(input: StartCardPracticeInput): Promise<PracticeSession> {
  return invoke<PracticeSession>("start_card_practice", { input });
}''','''export async function startCardPractice(input: StartCardPracticeInput): Promise<PracticeSession> {
  return invoke<PracticeSession>("start_card_practice", { input });
}

export async function getPracticeCardWindow(
  sessionId: string,
  startPosition: number,
  limit = 50,
): Promise<PracticeCardWindow> {
  return invoke<PracticeCardWindow>("get_practice_card_window", {
    input: { sessionId, startPosition, limit },
  });
}''')
write('src/ipc.ts',ipc)

# Analytics transport matches fixed current/history projections.
write('src/analytics-ipc.ts',r'''import { invoke } from "@tauri-apps/api/core";
import type { ReviewOutcome } from "./ipc";
export type AnalyticsRange="7d"|"30d"|"90d"|"1y"|"all";
export type TagAnalyticsStatus="building_data"|"needs_attention"|"on_track"|"strong";
export interface AnalyticsQuery{range:AnalyticsRange;deckIds:string[];tagIds:string[];highlightedSessionId?:string}
export interface AnalyticsScope{range:AnalyticsRange;startAt:number|null;endAt:number;bucketGranularity:"day"|"week"|"month";deckIds:string[];tagIds:string[];highlightedSessionId:string|null}
export interface AnalyticsSummary{currentDeckCount:number;sourceNoteCount:number;generatedCardCount:number;studyableCardCount:number;studyableDueCount:number;needsMorePractice:number;cardsPracticed:number;reviewCount:number;correctCount:number;correctRate:number|null;averageConfidence:number|null;averageResponseTimeMs:number|null;totalStudyTimeMs:number;sessionCount:number}
export interface DailyLimitUsage{newLimit:number;reviewLimit:number;newCompleted:number;reviewsCompleted:number;newRemaining:number;reviewsRemaining:number}
export interface RatingDistribution{again:number;hard:number;good:number;easy:number}
export interface TimelineBucket{startAt:number;endAt:number;cardsPracticed:number;reviewCount:number;newCards:number;rememberedCount:number;correctRate:number|null;averageConfidence:number|null;averageResponseTimeMs:number|null;studyTimeMs:number;sessionCount:number;containsHighlightedSession:boolean}
export interface DeckAnalytics{deckId:string;deckName:string;archived:boolean;sourceNoteCount:number;generatedCardCount:number;studyableDueCount:number;needsMorePractice:number;cardsPracticed:number;reviewCount:number;correctCount:number;correctRate:number|null;averageConfidence:number|null;averageResponseTimeMs:number|null;totalStudyTimeMs:number;lastPracticedAt:number|null}
export interface TagAnalytics{tagId:string;tagName:string;sourceNoteCount:number;generatedCardCount:number;studyableDueCount:number;needsMorePractice:number;cardsPracticed:number;reviewCount:number;correctCount:number;correctRate:number|null;averageConfidence:number|null;averageResponseTimeMs:number|null;totalStudyTimeMs:number;lastPracticedAt:number|null;trendDelta:number|null;status:TagAnalyticsStatus}
export interface RecentReview{id:string;deckId:string;deckName:string;cardId:string;frontText:string;tags:string[];outcome:ReviewOutcome;rating:1|2|3|4;responseTimeMs:number;reviewedAt:number;dueAfter:number;attributionSource:"captured"|"legacy_current_state_backfill"}
export interface SessionAnalytics{id:string;mode:"scheduled"|"extra"|"custom";deckNames:string[];startedAt:number;completedAt:number|null;durationMs:number;cardCount:number;ratedCount:number;correctCount:number;correctRate:number|null;studyTimeMs:number;highlighted:boolean}
export interface AnalyticsDimensionOption{id:string;name:string;archived:boolean;historicalOnly:boolean}
export interface AnalyticsDimensionOptions{decks:AnalyticsDimensionOption[];tags:AnalyticsDimensionOption[]}
export interface AttributionCoverage{capturedReviewCount:number;legacyBackfilledReviewCount:number}
export interface Analytics{scope:AnalyticsScope;summary:AnalyticsSummary;dailyLimits:DailyLimitUsage;ratingDistribution:RatingDistribution;timeline:TimelineBucket[];deckStats:DeckAnalytics[];tagStats:TagAnalytics[];recentReviews:RecentReview[];recentSessions:SessionAnalytics[];dimensions:AnalyticsDimensionOptions;attributionCoverage:AttributionCoverage}
export async function getAnalytics(input:AnalyticsQuery):Promise<Analytics>{return invoke<Analytics>("get_analytics",{input})}
export async function getRecentPracticeReviews():Promise<RecentReview[]>{return invoke<RecentReview[]>("get_recent_practice_reviews")}
''')

# Date-only manual scheduling is now resolved by Rust.
schedule=read('src/screens/cards/ScheduleActionModal.tsx')
schedule=schedule.replace('  type SelectionCard,','  type RescheduleIntent,\n  type SelectionCard,')
schedule=schedule.replace('''    const dueAt = action === "reschedule" ? resolveDueAt(dueChoice, customDate) : undefined;
    if (action === "reschedule" && dueAt === null) {''','''    const reschedule = action === "reschedule" ? resolveReschedule(dueChoice, customDate) : undefined;
    if (action === "reschedule" && reschedule === null) {''')
schedule=schedule.replace('const key = JSON.stringify({ action, cardIds, dueAt });','const key = JSON.stringify({ action, cardIds, reschedule });')
schedule=schedule.replace('        dueAt: dueAt ?? undefined,','        reschedule: reschedule ?? undefined,')
start=schedule.index('function resolveDueAt(')
schedule=schedule[:start]+'''function resolveReschedule(choice: string, customDate: string): RescheduleIntent | null {
  if (choice === "today") return { kind: "dueNow" };
  if (choice === "tomorrow") return { kind: "tomorrow" };
  if (choice === "3_days") return { kind: "calendarDays", days: 3 };
  if (choice === "1_week") return { kind: "calendarDays", days: 7 };
  if (choice === "custom" && /^\\d{4}-\\d{2}-\\d{2}$/u.test(customDate)) {
    return { kind: "calendarDate", date: customDate };
  }
  return null;
}
'''
write('src/screens/cards/ScheduleActionModal.tsx',schedule)

# Global deck count semantics and practice window IPC imports.
for path in ['src/App.tsx','src/screens/DecksScreen.tsx','src/screens/CardsScreen.tsx','src/screens/PracticeScreen.tsx','src/screens/practice/PracticeSetup.tsx','src/screens/AnalyticsScreen.tsx']:
    text=read(path).replace('.cardCount','.generatedCardCount').replace('.dueCount','.studyableDueCount')
    write(path,text)

# Practice screen: tag scope and bounded window prefetch.
practice=read('src/screens/PracticeScreen.tsx')
practice=practice.replace('  getPracticeWorkload,','  getPracticeCardWindow,\n  getPracticeWorkload,')
practice=practice.replace('  const [temporaryCardIds, setTemporaryCardIds] = useState<string[]>([]);','  const [temporaryCardIds, setTemporaryCardIds] = useState<string[]>([]);\n  const [selectedTagIds, setSelectedTagIds] = useState<string[]>([]);\n  const [isLoadingWindow, setIsLoadingWindow] = useState(false);')
practice=practice.replace('setWorkload(await getPracticeWorkload(deckIds));','setWorkload(await getPracticeWorkload(deckIds, selectedTagIds));')
practice=practice.replace('  }, []);\n\n  const beginSession = useCallback(', '  }, [selectedTagIds]);\n\n  const beginSession = useCallback(',1)
practice=practice.replace('''      mode: "scheduled" | "extra",
    ): Promise<void> => {''','''      mode: "scheduled" | "extra",
      tagIds: string[] = selectedTagIds,
    ): Promise<void> => {''',1)
practice=practice.replace('const key = selectedStartKey(deckIds, requestedShuffle, mode);','const key = selectedStartKey(deckIds, tagIds, requestedShuffle, mode);',1)
practice=practice.replace('            deckIds,\n            shuffle:', '            deckIds,\n            tagIds,\n            shuffle:',1)
practice=practice.replace('    [beginCardSession,\n    beginSession,','    [beginCardSession,\n    beginSession,')
# Support launch tag IDs once App adds them.
practice=practice.replace('    if (launchRequest.cardIds.length > 0) {','    setSelectedTagIds(launchRequest.tagIds ?? []);\n    if (launchRequest.cardIds.length > 0) {',1)
practice=practice.replace('void beginSession(eligible, settings.practiceShuffle, "scheduled");','void beginSession(eligible, settings.practiceShuffle, "scheduled", launchRequest.tagIds ?? []);')
# Prefetch next window.
marker='  async function revealAnswer(): Promise<void> {'
if 'async function loadNextWindow' not in practice:
    addition='''  async function loadNextWindow(): Promise<void> {
    if (!session || !session.hasMore || isLoadingWindow) return;
    setIsLoadingWindow(true);
    try {
      const window = await getPracticeCardWindow(session.id, session.cards.length, 50);
      setSession((current) => current && current.id === session.id
        ? { ...current, cards: [...current.cards, ...window.cards], hasMore: window.hasMore }
        : current);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsLoadingWindow(false);
    }
  }

  useEffect(() => {
    if (session?.hasMore && position >= Math.max(0, session.cards.length - 5)) {
      void loadNextWindow();
    }
  }, [position, session?.id, session?.cards.length, session?.hasMore]);

'''
    practice=practice.replace(marker,addition+marker,1)
# Fingerprint helper signature near bottom.
practice=practice.replace('function selectedStartKey(deckIds: string[], shuffle: boolean, mode: string): string {','function selectedStartKey(deckIds: string[], tagIds: string[], shuffle: boolean, mode: string): string {')
practice=practice.replace('return JSON.stringify({ deckIds: [...deckIds].sort(), shuffle, mode });','return JSON.stringify({ deckIds: [...deckIds].sort(), tagIds: [...tagIds].sort(), shuffle, mode });')
write('src/screens/PracticeScreen.tsx',practice)

# App launch request carries tag scope, and Analytics can launch native tag-scoped practice.
app=read('src/App.tsx')
app=app.replace('  cardIds: string[];\n  autoStart: boolean;','  cardIds: string[];\n  tagIds: string[];\n  autoStart: boolean;')
app=app.replace('cardIds: [], autoStart:', 'cardIds: [], tagIds: [], autoStart:')
app=app.replace('cardIds, autoStart: true', 'cardIds, tagIds: [], autoStart: true')
if 'function practiceScope' not in app:
    anchor='  function viewSessionAnalytics('
    addition='''  function practiceScope(deckIds: string[], tagIds: string[]): void {
    if (!canLeaveCurrentSection()) return;
    const requestId = practiceRequestSequence.current;
    practiceRequestSequence.current += 1;
    setPracticeLaunch({ requestId, deckIds, cardIds: [], tagIds, autoStart: true });
    setSection("practice");
  }

'''
    app=app.replace(anchor,addition+anchor,1)
app=app.replace('onPracticeCards={practiceCards}\n              showToast', 'onPracticeCards={practiceCards}\n              onPracticeScope={practiceScope}\n              showToast')
write('src/App.tsx',app)

# Analytics uses backend dimensions/current names and starts tag scope without a 200-card cap.
analytics=read('src/screens/AnalyticsScreen.tsx')
analytics=analytics.replace('  listMatchingCards,\n','')
analytics=analytics.replace('  type Deck,\n','  type Deck,\n')
analytics=analytics.replace('  onPracticeCards: (cardIds: string[]) => void;','  onPracticeCards: (cardIds: string[]) => void;\n  onPracticeScope: (deckIds: string[], tagIds: string[]) => void;')
analytics=analytics.replace('  onPracticeCards,\n  showToast,','  onPracticeCards,\n  onPracticeScope,\n  showToast,')
start=analytics.index('  async function practiceDueTag(')
end=analytics.index('\n\n  const selectedDeckNames',start)
analytics=analytics[:start]+'''  async function practiceDueTag(tag: TagAnalytics): Promise<void> {
    if (practicingTagId !== null) return;
    setPracticingTagId(tag.tagId);
    try {
      onPracticeScope(deckIds, [...new Set([...tagIds, tag.tagId])]);
    } finally {
      setPracticingTagId(null);
    }
  }'''+analytics[end:]
analytics=analytics.replace('analytics.summary.dueNow','analytics.summary.studyableDueCount')
analytics=analytics.replace('analytics.summary.cardCount','analytics.summary.generatedCardCount')
analytics=analytics.replace('tag.cardCount','tag.sourceNoteCount').replace('tag.dueCount','tag.studyableDueCount')
analytics=analytics.replace('deck.cardCount','deck.generatedCardCount').replace('deck.dueCount','deck.studyableDueCount')
# Filter options switch to backend dimensions after load.
analytics=analytics.replace('options={decks.map((deck) => ({ id: deck.id, name: deck.name, detail: `${deck.generatedCardCount} cards` }))}','options={(analytics?.dimensions.decks ?? decks.map((deck) => ({ ...deck, archived: false, historicalOnly: false }))).map((deck) => ({ id: deck.id, name: deck.name, detail: deck.archived ? "Archived · history retained" : "Current deck" }))}')
write('src/screens/AnalyticsScreen.tsx',analytics)

# Deck screen imports projected rows and archive controls. Keep the existing layout,
# but all list rows are bounded summaries and full cards load only on Edit.
decks=read('src/screens/DecksScreen.tsx')
decks=decks.replace('  createDeck,','  archiveDeck,\n  createDeck,')
decks=decks.replace('  getDeck,','  getDeck,\n  getOrganizedCard,')
decks=decks.replace('  listCards,','  listArchivedDecks,\n  listDeckCards,')
decks=decks.replace('  undoDeletion,','  undoDeletion,\n  unarchiveDeck,')
decks=decks.replace('  type Flashcard,','  type DeckCardSummary,\n  type OrganizedCard,')
decks=decks.replace('const [cards, setCards] = useState<Flashcard[]>([]);','const [cards, setCards] = useState<DeckCardSummary[]>([]);\n  const [archivedDecks, setArchivedDecks] = useState<Deck[]>([]);\n  const [cardTotal, setCardTotal] = useState(0);')
decks=decks.replace('const [editingCard, setEditingCard] = useState<Flashcard | null>(null);','const [editingCard, setEditingCard] = useState<OrganizedCard | null>(null);')
decks=decks.replace('const [deck, loadedCards] = await Promise.all([getDeck(deckId), listCards(deckId)]);','const [deck, page] = await Promise.all([getDeck(deckId), listDeckCards(deckId, "", 50, 0)]);\n      const loadedCards = page.cards;\n      setCardTotal(page.totalCount);')
decks=decks.replace('const [deck, loadedCards] = await Promise.all([getDeck(deckId), listCards(deckId)]);','const [deck, page] = await Promise.all([getDeck(deckId), listDeckCards(deckId, search, 50, 0)]);\n      const loadedCards = page.cards;\n      setCardTotal(page.totalCount);')
decks=decks.replace('const visibleCards = useMemo(() => {','const visibleCards = useMemo(() => {')
decks=decks.replace('`${card.frontText} ${card.backText}`','`${card.questionExcerpt} ${card.answerExcerpt}`')
decks=decks.replace('async function handleDeleteCard(card: Flashcard)', 'async function handleDeleteCard(card: DeckCardSummary)')
decks=decks.replace('card.frontText || "Image card"','card.questionExcerpt || "Image card"')
decks=decks.replace('summarize(card.frontText, card.assets.length > 0)','summarize(card.questionExcerpt, card.imageCount > 0)')
decks=decks.replace('summarize(card.backText, card.assets.length > 0)','summarize(card.answerExcerpt, card.imageCount > 0)')
decks=decks.replace('card.assets.length > 0\n                            ? `${card.assets.length} image${\n                                card.assets.length === 1 ? "" : "s"\n                              }`','card.imageCount > 0\n                            ? `${card.imageCount} image${card.imageCount === 1 ? "" : "s"}`')
# Edit clicks lazily hydrate.
decks=decks.replace('''setEditingCard(card);
                          setView("editor");''','''void getOrganizedCard(card.noteId).then((loaded) => {
                            setEditingCard(loaded);
                            setView("editor");
                          }).catch((error: unknown) => showToast("error", getErrorMessage(error)));''')
# Add archived deck loading on library refresh/mount.
if 'void listArchivedDecks()' not in decks:
    decks=decks.replace('  const selectedDeckId = selectedDeck?.id ?? null;','  const selectedDeckId = selectedDeck?.id ?? null;\n\n  useEffect(() => { void listArchivedDecks().then(setArchivedDecks).catch(() => setArchivedDecks([])); }, [decks]);')
# Archive action and basic archived section before modal.
if 'handleArchiveDeck' not in decks:
    anchor='  async function handleDeleteDeck(deck: Deck): Promise<void> {'
    addition='''  async function handleArchiveDeck(deck: Deck): Promise<void> {
    try {
      await archiveDeck(deck.id);
      showToast("success", `Archived “${deck.name}”. Its cards are suspended.`);
      await refreshDeckListAfterMutation();
    } catch (error: unknown) { showToast("error", getErrorMessage(error)); }
  }

  async function handleUnarchiveDeck(deck: Deck): Promise<void> {
    try {
      await unarchiveDeck(deck.id);
      showToast("success", `Restored “${deck.name}”.`);
      await refreshDeckListAfterMutation();
    } catch (error: unknown) { showToast("error", getErrorMessage(error)); }
  }

'''
    decks=decks.replace(anchor,addition+anchor,1)
decks=decks.replace('<button\n                      type="button"\n                      className="danger-text"\n                      onClick={() => void handleDeleteDeck(deck)}','<button type="button" onClick={() => void handleArchiveDeck(deck)}>Archive</button>\n                    <button\n                      type="button"\n                      className="danger-text"\n                      onClick={() => void handleDeleteDeck(deck)}')
# Some JSX exact anchor may not match; insert archived panel before deckDialog.
if 'Archived decks' not in decks:
    anchor='      {deckDialog ? ('
    panel='''      {view === "library" && archivedDecks.length > 0 ? (
        <section className="archived-decks-panel">
          <h2>Archived decks</h2>
          <p>Archived deck history remains in Analytics; its cards stay suspended.</p>
          {archivedDecks.map((deck) => (
            <article key={deck.id} className="deck-card archived">
              <strong>{deck.name}</strong>
              <span>{deck.generatedCardCount} cards</span>
              <button type="button" onClick={() => void handleUnarchiveDeck(deck)}>Unarchive</button>
              <button type="button" className="danger-text" onClick={() => void handleDeleteDeck(deck)}>Delete</button>
            </article>
          ))}
        </section>
      ) : null}

'''
    decks=decks.replace(anchor,panel+anchor,1)
write('src/screens/DecksScreen.tsx',decks)

# Template screens use summaries, fetching one full graph only when opened.
templates=read('src/screens/TemplatesScreen.tsx')
templates=templates.replace('  listCardTemplates,','  getCardTemplate,\n  listCardTemplateSummaries,')
templates=templates.replace('  type CardTemplate,','  type CardTemplate,\n  type CardTemplateSummary,')
templates=templates.replace('const [templates, setTemplates] = useState<CardTemplate[]>([]);','const [templates, setTemplates] = useState<CardTemplateSummary[]>([]);')
templates=templates.replace('setTemplates(await listCardTemplates());','setTemplates(await listCardTemplateSummaries());')
templates=templates.replace('function beginEdit(template: CardTemplate): void {\n    void openDraft(draftFromTemplate(template));\n  }','async function beginEdit(template: CardTemplateSummary): Promise<void> {\n    try { await openDraft(draftFromTemplate(await getCardTemplate(template.id))); }\n    catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }\n  }')
templates=templates.replace('async function removeTemplate(template: CardTemplate)', 'async function removeTemplate(template: CardTemplateSummary)')
templates=templates.replace('template.fields.length','template.fieldCount').replace('template.rules.length','template.generatedCardRuleCount')
templates=templates.replace('onClick={() => beginEdit(template)}','onClick={() => void beginEdit(template)}')
write('src/screens/TemplatesScreen.tsx',templates)

editor=read('src/screens/CardEditorScreen.tsx')
editor=editor.replace('  listCardTemplates,','  getCardTemplate,\n  listCardTemplateSummaries,')
editor=editor.replace('  type CardTemplate,','  type CardTemplate,\n  type CardTemplateSummary,')
editor=editor.replace('const [templates, setTemplates] = useState<CardTemplate[]>([]);','const [templates, setTemplates] = useState<CardTemplateSummary[]>([]);')
editor=editor.replace('void listCardTemplates()','void listCardTemplateSummaries()')
# selected custom detail now separate.
editor=editor.replace('''  const selectedCustomTemplate = templates.find(
    (template) => template.id === selectedTemplateKey,
  );''','''  const [selectedCustomTemplate, setSelectedCustomTemplate] = useState<CardTemplate | undefined>();''')
if 'void getCardTemplate(selectedTemplateKey)' not in editor:
    marker='  useEffect(() => {\n    if (!selectedCustomTemplate) {'
    effect='''  useEffect(() => {
    if (selectedTemplateKey === "basic" || selectedTemplateKey === "cloze") {
      setSelectedCustomTemplate(undefined);
      return;
    }
    let current = true;
    void getCardTemplate(selectedTemplateKey)
      .then((template) => { if (current) setSelectedCustomTemplate(template); })
      .catch((error: unknown) => { if (current) setErrorMessage(getErrorMessage(error)); });
    return () => { current = false; };
  }, [selectedTemplateKey]);

'''
    editor=editor.replace(marker,effect+marker,1)
write('src/screens/CardEditorScreen.tsx',editor)

# Manifest verifier follows the new bounded command contracts.
manifest=read('scripts/verify-project-manifests.mjs')
manifest=manifest.replace('"list_cards", "delete_card",','"list_deck_cards", "delete_card",')
manifest=manifest.replace('"list_card_templates",','"list_card_template_summaries",')
for command in ['list_archived_decks','archive_deck','unarchive_deck','get_practice_card_window']:
    if f'"{command}"' not in manifest:
        manifest=manifest.replace('"list_decks",',f'"list_decks", "{command}",',1)
write('scripts/verify-project-manifests.mjs',manifest)

print('Applied frontend deck, template, practice-window, Analytics, and date-intent contracts.')
